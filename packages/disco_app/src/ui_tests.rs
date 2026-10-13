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
fn home_app() -> impl IntoElement {
    let station = use_init_radio_station::<AppState, AppChannel>(AppState::initial);
    let actions = use_hook(move || {
        let actions = Actions::new(station);
        actions.refresh_instances();
        actions
    });
    use_provide_actions(&actions);
    rect()
        .width(Size::fill())
        .height(Size::fill())
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
