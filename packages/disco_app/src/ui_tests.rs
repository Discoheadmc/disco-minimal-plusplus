//! UI testleri — gerçek render + tıklama simülasyonu (freya-testing).
//!
//! Kilometre Taşı 2'deki sekme mimarisi, koşullu dallarda serbest fonksiyon
//! içinde hook kullanımının Freya'da paniklediği için `Component`'lara
//! sarındı; bu testler o mimarinin kırılmadığını kanıtlar.

use std::any::Any;

use freya::elements::label::LabelElement;
use freya::prelude::*;
use freya::radio::use_init_radio_station;
use freya_testing::prelude::*;

use crate::actions::{use_provide_actions, Actions};
use crate::state::{AppChannel, AppState};
use crate::view::Home;

/// Ana pencerenin birebir kopyası (ağ çağrısı yapan manifest yüklemesi hariç).
/// Test düzeneği köke pencere kısıtı vermediği için fill() çözümlenemez;
/// kök sabit piksel boyutuyla verilir (gerçek pencerenin davranışı).
fn home_app() -> impl IntoElement {
    let station = use_init_radio_station::<AppState, AppChannel>(AppState::initial);
    let actions = use_hook(move || {
        let actions = Actions::new(station);
        actions.refresh_instances();
        actions
    });
    use_provide_actions(&actions);
    rect()
        .width(Size::px(1180.))
        .height(Size::px(760.))
        .background(crate::theme::bg())
        .child(Home)
}

/// Tanı uygulaması: manifest hazır kabul edilir (ağsız, deterministik).
fn home_app_seeded() -> impl IntoElement {
    let station = use_init_radio_station::<AppState, AppChannel>(move || {
        let mut state = AppState::initial();
        state.manifest = crate::state::ManifestState::Ready {
            latest: "1.21.9".into(),
            releases: vec![
                "1.21.9".into(),
                "1.21.8".into(),
                "1.20.6".into(),
            ],
        };
        state
    });
    let actions = use_hook(move || {
        let actions = Actions::new(station);
        actions.refresh_instances();
        actions
    });
    use_provide_actions(&actions);
    rect()
        .width(Size::px(1180.))
        .height(Size::px(760.))
        .background(crate::theme::bg())
        .child(Home)
}

fn launch_home() -> TestingRunner {
    launch_test(home_app)
}

/// Ekrandaki tüm label metinlerini toplar.
fn label_texts(test: &TestingRunner) -> Vec<String> {
    test.find_many(|_node, element| {
        (element as &dyn Any)
            .downcast_ref::<LabelElement>()
            .map(|label| label.text.to_string())
    })
}

#[test]
fn store_tab_switches_without_panic() {
    // Gerçek main() gibi: reqwest (show_popular) için tokio bağlamı şart.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _runtime_guard = rt.enter();

    let mut test = launch_home();
    test.sync_and_update();

    // ilk kare: Instance'lar sekmesi görünür olmalı
    let before = label_texts(&test);
    assert!(
        before.iter().any(|text| text.contains("Instance'lar")),
        "ilk sekme görünmüyor, metinler: {before:?}"
    );
    assert!(
        !before.iter().any(|text| text.contains("Modrinth Mağazası")),
        "mağaza paneli hiç istenmeden açık olmamalı"
    );

    // "Mağaza" butonuna tıkla (başlık çubuğu: logo, isim, Instance'lar, Mağaza)
    test.click_cursor((390., 29.));
    test.sync_and_update();

    let after = label_texts(&test);
    assert!(
        after.iter().any(|text| text.contains("Modrinth Mağazası")),
        "mağaza paneli açılmadı — tıklama hedefi kaymış olabilir, metinler: {after:?}"
    );

    // "Instance'lar"a geri dön — geçiş paniklemiyorsa buraya ulaşırız
    test.click_cursor((290., 29.));
    test.sync_and_update();

    let back = label_texts(&test);
    assert!(
        back.iter().any(|text| text.contains("Yeni Instance")),
        "instance paneli geri dönmedi, metinler: {back:?}"
    );
    assert!(
        !back.iter().any(|text| text.contains("Modrinth Mağazası")),
        "mağaza paneli kapanmıyor"
    );
}

/// Etiket metnine göre düğümün merkez koordinatını bulur.
fn label_center(test: &TestingRunner, needle: &str) -> Option<(f64, f64)> {
    test.find(|node, element| {
        let label = (element as &dyn Any).downcast_ref::<LabelElement>()?;
        label.text.contains(needle).then(|| {
            let center = node.layout().area.center();
            (center.x as f64, center.y as f64)
        })
    })
}

/// Etiket metninin TAM eşleşmesiyle merkez bulur ("1.21.9" çipinde
/// "Minecraft sürümü (en yeni: 1.21.9)" gibi kapsayan etiketlerden kaçınır).
fn label_center_exact(test: &TestingRunner, needle: &str) -> Option<(f64, f64)> {
    test.find(|node, element| {
        let label = (element as &dyn Any).downcast_ref::<LabelElement>()?;
        (label.text == needle).then(|| {
            let center = node.layout().area.center();
            (center.x as f64, center.y as f64)
        })
    })
}

/// Tanı: tüm instance oluşturma akışı (ad yaz → çip seç → Oluştur) tıklama
/// simülasyonuyla adım adım doğrulanır. Manifest hazır kabul edilir.
#[test]
fn diagnose_instance_creation_flow() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _runtime_guard = rt.enter();

    let (mut test, _) = TestingRunner::new(
        home_app_seeded,
        Size2D::new(1180., 760.),
        |runner| {
            runner.provide_root_context(|| ());
        },
        1.,
    );
    test.sync_and_update();
    let root_area = test
        .find(|node, _element| Some(node.layout().area))
        .expect("kok dugun");
    eprintln!("--- KÖK ALAN: {root_area:?}");

    // 0) TANı: tüm etiketlerin raporlanan konumlarını dök
    let positions: Vec<(String, f64, f64)> = test.find_many(|node, element| {
        (element as &dyn Any)
            .downcast_ref::<LabelElement>()
            .map(|label| {
                let center = node.layout().area.center();
                (label.text.to_string(), center.x as f64, center.y as f64)
            })
    });
    eprintln!("--- ETİKET KONUMLARI ---");
    for (text, x, y) in &positions {
        eprintln!("  [{text}] x={x:.1} y={y:.1}");
    }
    eprintln!("--- ETİKET KONUMLARI döküldü ---");

    // 1) isim girişine tıkla ve yaz
    let name_label = label_center(&test, "Instance adı").expect("isim etiketi bulunamadı");
    eprintln!("isim etiketi: {name_label:?}");
    test.click_cursor((name_label.0, name_label.1 + 22.));
    test.sync_and_update();
    test.write_text("TestKur");
    test.sync_and_update();

    // 2) sürüm çipine tıkla (tam eşleşme: "1.21.9" çipi, "(en yeni: …)" değil)
    let chip = label_center_exact(&test, "1.21.9").expect("sürüm çipi bulunamadı");
    eprintln!("çip: {chip:?}");
    test.click_cursor(chip);
    test.sync_and_update();

    // 3) Oluştur'a tıkla
    let create_btn = label_center(&test, "Oluştur").expect("Oluştur butonu bulunamadı");
    eprintln!("oluştur: {create_btn:?}");
    test.click_cursor(create_btn);
    test.sync_and_update();
    // spawn_forever'daki create görevi bitsin
    test.poll_n(std::time::Duration::from_millis(50), 10);
    test.sync_and_update();

    // 4) instance listesinde göründü mü?
    let texts = label_texts(&test);
    eprintln!("--- SON METİNLER: {texts:?}");
    assert!(
        texts.iter().any(|text| text.contains("TestKur")),
        "instance oluşturulamadı: {texts:?}"
    );
    assert!(
        texts.iter().any(|text| text.contains("Instance'lar (1)")),
        "liste sayacı güncellenmedi: {texts:?}"
    );

    // temizlik: test instance'ı gerçek veri dizininden sil
    let dir = disco_common::paths::instances_dir()
        .expect("instances dizini")
        .join("TestKur");
    let _ = std::fs::remove_dir_all(dir);
}

/// Tanı: pencere kontrol butonlarının pikselleri — ekran görüntüsünde
/// sağ üst bölgede açık renk ikon var mı?
#[test]
fn diagnose_window_controls_screenshot() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let _runtime_guard = rt.enter();

    let (mut test, _) = TestingRunner::new(
        home_app_seeded,
        Size2D::new(1180., 760.),
        |runner| {
            runner.provide_root_context(|| ());
        },
        1.,
    );
    test.sync_and_update();
    // SvgViewer asenkran rasterizasyonuna zaman tanı
    test.poll_n(std::time::Duration::from_millis(50), 20);
    test.sync_and_update();

    let shot = std::env::temp_dir().join("dm-controls.png");
    test.render_to_file(shot.clone());
    eprintln!("screenshot: {}", shot.display());
}
