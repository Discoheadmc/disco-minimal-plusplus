//! Ana ekran.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::actions::use_dispatch;
use crate::components::{hex_to_color, ButtonVariant, DButton, ACCENTS};
use crate::state::{AppChannel, LaunchState, LoginUiState, ManifestState, StoreHit, StoreState};
use crate::theme;
use disco_core::Instance;

#[derive(Clone, Copy, PartialEq)]
pub enum Tab {
    Instances,
    Store,
}

#[derive(PartialEq)]
pub struct Home;

impl Component for Home {
    fn render(&self) -> impl IntoElement {
        let actions = use_dispatch();
        let instances = use_radio(AppChannel::Instances).read().instances.clone();
        let manifest = use_radio(AppChannel::Manifest).read().manifest.clone();
        let launch = use_radio(AppChannel::Launch).read().launch.clone();
        let player = use_radio(AppChannel::Settings).read().settings.player_name.clone();
        let store = use_radio(AppChannel::Store).read().store.clone();

        let busy = !matches!(launch, LaunchState::Idle | LaunchState::Failed(_));

        let tab = use_state(|| Tab::Instances);
        let active = *tab.read();

        // Sekme içerikleri AYRI Component'lardır: her birinin kendi hook
        // kapsamı vardır. Koşullu dallarda serbest fonksiyon içi hook
        // kullanmak Freya'da panikle sonuçlanır.
        let content = match active {
            Tab::Instances => InstancesTab {
                instances,
                manifest,
                player,
                busy,
            }
            .into_element(),
            Tab::Store => StoreTab { store }.into_element(),
        };

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .background(theme::bg())
            .child(header(tab, &actions))
            .child(content)
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .padding(Gaps::new(16., 20., 8., 20.))
                    .spacing(16.)
                    .cross_align(Alignment::Center)
                    .child(account_card(&actions))
                    .child(theme_card(&actions)),
            )
            .child(status_bar(&launch))
    }
}

/// "Instance'lar" sekmesi — kendi hook kapsamında yaşar.
#[derive(Clone, PartialEq)]
pub struct InstancesTab {
    pub instances: Vec<Instance>,
    pub manifest: ManifestState,
    pub player: String,
    pub busy: bool,
}

impl Component for InstancesTab {
    fn render(&self) -> impl IntoElement {
        let actions = use_dispatch();
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .padding(Gaps::new(20., 20., 0., 20.))
            .spacing(16.)
            .child(instances_panel(&self.instances, self.busy, &actions))
            .child(create_panel(&self.manifest, &self.player, self.busy, &actions))
    }
}

/// "Mağaza" sekmesi — kendi hook kapsamında yaşar.
#[derive(Clone, PartialEq)]
pub struct StoreTab {
    pub store: StoreState,
}

impl Component for StoreTab {
    fn render(&self) -> impl IntoElement {
        let actions = use_dispatch();
        rect()
            .horizontal()
            .width(Size::fill())
            .height(Size::fill())
            .padding(Gaps::new(20., 20., 0., 20.))
            .spacing(16.)
            .child(store_panel(&self.store, &actions))
    }
}

fn header(tab: State<Tab>, actions: &crate::actions::Actions) -> impl IntoElement {
    let active = *tab.read();
    let mut tab_instances = tab;
    let mut tab_store = tab;
    let store_actions = actions.clone();

    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new(12., 0., 12., 20.))
        .spacing(12.)
        .cross_align(Alignment::Center)
        .background(theme::surface())
        .window_drag()
        .child(
            rect()
                .width(Size::px(34.))
                .height(Size::px(34.))
                .corner_radius(CornerRadius::new_all(10.))
                .background(crate::components::use_accent().base)
                .main_align(Alignment::Center)
                .cross_align(Alignment::Center)
                .child(
                    label()
                        .text("D")
                        .font_size(18.)
                        .font_weight(FontWeight::BOLD)
                        .color(Color::WHITE),
                ),
        )
        .child(
            label()
                .text("Disco Minimal++")
                .font_size(19.)
                .font_weight(FontWeight::BOLD)
                .color(theme::fg()),
        )
        .child(
            DButton::new("Instance'lar")
                .variant(if active == Tab::Instances {
                    ButtonVariant::Selected
                } else {
                    ButtonVariant::Ghost
                })
                .on_press(move |_| tab_instances.set(Tab::Instances))
                .into_element(),
        )
        .child(
            DButton::new("Mağaza")
                .variant(if active == Tab::Store {
                    ButtonVariant::Selected
                } else {
                    ButtonVariant::Ghost
                })
                .on_press(move |_| {
                    tab_store.set(Tab::Store);
                    // boşsa popüler listesini getir (loaded guard'ı içeride)
                    store_actions.show_popular();
                })
                .into_element(),
        )
        .child(
            rect()
                .width(Size::fill())
                .main_align(Alignment::End)
                .padding(Gaps::new(0., 12., 0., 0.))
                .child(
                    label()
                        .text(format!("v{} — Rust + Freya", disco_common::constants::VERSION))
                        .font_size(12.)
                        .color(theme::fg_muted()),
                ),
        )
        .child(window_controls())
}

/// Borderless pencere kontrolleri: küçült / büyült / kapat.
/// Başlık çubuğunun sağ ucunda, hover'da vurgulanan standart butonlar.
fn window_controls() -> impl IntoElement {
    let minimize = move |_: Event<PressEventData>| {
        Platform::get().with_window(Platform::window_id(), |window| {
            window.set_minimized(true);
        });
    };
    let maximize = move |_: Event<PressEventData>| {
        Platform::get().with_window(Platform::window_id(), |window| {
            window.set_maximized(!window.is_maximized());
        });
    };
    let close = move |_: Event<PressEventData>| {
        Platform::get().close_window(Platform::window_id());
    };

    rect()
        .horizontal()
        .height(Size::px(34.))
        .child(TitlebarButton::new(TitlebarAction::Minimize).on_press(minimize))
        .child(TitlebarButton::new(TitlebarAction::Maximize).on_press(maximize))
        .child(TitlebarButton::new(TitlebarAction::Close).on_press(close))
}

fn instances_panel(
    instances: &[Instance],
    busy: bool,
    actions: &crate::actions::Actions,
) -> impl IntoElement {
    let mut cards: Vec<Element> = Vec::new();

    if instances.is_empty() {
        cards.push(
            label()
                .text("Henüz instance yok. Sağdaki panelden oluştur.")
                .font_size(13.)
                .color(theme::fg_muted())
                .into_element(),
        );
    } else {
        for instance in instances {
            cards.push(instance_card(instance, busy, actions).into_element());
        }
    }

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .padding(Gaps::new(16., 16., 16., 16.))
        .spacing(12.)
        .corner_radius(CornerRadius::new_all(12.))
        .background(theme::surface())
        .overflow(Overflow::Clip)
        .child(
            label()
                .text(format!("Instance'lar ({})", instances.len()))
                .font_size(15.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(theme::fg()),
        )
        .children(cards)
}

fn instance_card(
    instance: &Instance,
    busy: bool,
    actions: &crate::actions::Actions,
) -> impl IntoElement {
    let launch_actions = actions.clone();
    let delete_actions = actions.clone();
    let name = instance.name.clone();
    let name_for_delete = instance.name.clone();

    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new(12., 12., 12., 12.))
        .spacing(10.)
        .cross_align(Alignment::Center)
        .corner_radius(CornerRadius::new_all(10.))
        .background(theme::surface_raised())
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(2.)
                .child(
                    label()
                        .text(instance.name.clone())
                        .font_size(14.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(theme::fg()),
                )
                .child(
                    label()
                        .text(instance.version_id.clone())
                        .font_size(12.)
                        .color(theme::fg_muted()),
                ),
        )
        .child(
            DButton::new("Başlat")
                .on_press(move |_| {
                    launch_actions.launch_instance(name.clone());
                })
                .enabled(!busy)
                .into_element(),
        )
        .child(
            DButton::new("✕")
                .ghost()
                .on_press(move |_| {
                    delete_actions.delete_instance(name_for_delete.clone());
                })
                .into_element(),
        )
}

fn create_panel(
    manifest: &ManifestState,
    player: &str,
    busy: bool,
    actions: &crate::actions::Actions,
) -> impl IntoElement {
    let instance_name = use_state(String::new);
    let selected_version = use_state(|| None::<String>);
    let player_name = use_state(move || player.to_string());

    let versions = match manifest {
        ManifestState::Ready { releases, .. } => releases.clone(),
        ManifestState::Loading => Vec::new(),
        ManifestState::Failed(_) => Vec::new(),
    };
    let latest = match manifest {
        ManifestState::Ready { latest, .. } => latest.clone(),
        _ => String::new(),
    };

    let mut version_chips: Vec<Element> = Vec::new();
    if versions.is_empty() {
        version_chips.push(
            label()
                .text(match manifest {
                    ManifestState::Loading => "Sürümler yükleniyor…".to_string(),
                    ManifestState::Failed(err) => format!("Sürümler alınamadı: {err}"),
                    _ => unreachable!(),
                })
                .font_size(12.)
                .color(theme::fg_muted())
                .into_element(),
        );
    } else {
        for (index, version) in versions.iter().enumerate() {
            let version = version.clone();
            let is_selected = *selected_version.read() == Some(version.clone());
            let mut chip_state = selected_version.clone();
            version_chips.push(
                DButton::new(version.clone())
                    .variant(if is_selected {
                        ButtonVariant::Selected
                    } else {
                        ButtonVariant::Ghost
                    })
                    .on_press(move |_| {
                        chip_state.set(Some(version.clone()));
                    })
                    .into_element(),
            );
            let _ = index; // chips satırı ileride kaydırma kazanacak
        }
    }

    let chosen_version = (*selected_version.read()).clone();
    let typed_name = (*instance_name.read()).clone();
    let can_create = !typed_name.trim().is_empty() && chosen_version.is_some() && !busy;

    let create_actions = actions.clone();
    let name_actions = actions.clone();

    rect()
        .vertical()
        .width(Size::px(320.))
        .height(Size::fill())
        .padding(Gaps::new(16., 16., 16., 16.))
        .spacing(14.)
        .corner_radius(CornerRadius::new_all(12.))
        .background(theme::surface())
        .overflow(Overflow::Clip)
        .child(
            label()
                .text("Yeni Instance")
                .font_size(15.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(theme::fg()),
        )
        .child(
            rect()
                .vertical()
                .spacing(6.)
                .width(Size::fill())
                .child(
                    label()
                        .text("Instance adı")
                        .font_size(12.)
                        .color(theme::fg_muted()),
                )
                .child(
                    Input::new(instance_name)
                        .placeholder("ör. Survival")
                        .padding(Gaps::new_symmetric(8., 12.))
                        .corner_radius(CornerRadius::new_all(8.))
                        .background(theme::surface_raised())
                        .border_fill(theme::bg())
                        .color(theme::fg()),
                ),
        )
        .child(
            rect()
                .vertical()
                .spacing(6.)
                .width(Size::fill())
                .child(
                    label()
                        .text(if latest.is_empty() {
                            "Minecraft sürümü".to_string()
                        } else {
                            format!("Minecraft sürümü (en yeni: {latest})")
                        })
                        .font_size(12.)
                        .color(theme::fg_muted()),
                )
                .child(
                    rect()
                        .vertical()
                        .width(Size::fill())
                        .spacing(4.)
                        .children(version_chips),
                ),
        )
        .child(
            DButton::new("Oluştur")
                .enabled(can_create)
                .on_press(move |_| {
                    let version = chosen_version.clone().unwrap_or_default();
                    create_actions.create_instance(typed_name.clone(), version);
                })
                .into_element(),
        )
        .child(
            rect()
                .vertical()
                .spacing(6.)
                .width(Size::fill())
                .child(
                    label()
                        .text("Oyuncu adı (çevrimdışı)")
                        .font_size(12.)
                        .color(theme::fg_muted()),
                )
                .child(
                    Input::new(player_name)
                        .placeholder("Player")
                        .padding(Gaps::new_symmetric(8., 12.))
                        .corner_radius(CornerRadius::new_all(8.))
                        .background(theme::surface_raised())
                        .border_fill(theme::bg())
                        .color(theme::fg())
                        .on_submit(move |name: String| {
                            name_actions.set_player_name(name);
                        }),
                ),
        )
}

fn status_bar(launch: &LaunchState) -> impl IntoElement {
    let (text, color) = match launch {
        LaunchState::Idle => ("Hazır.".to_string(), theme::fg_muted()),
        LaunchState::Working { stage, done, total } => match (done, total) {
            (Some(done), Some(total)) => (
                format!("{stage} — {done}/{total} dosya"),
                theme::fg(),
            ),
            _ => (stage.clone(), theme::fg()),
        },
        LaunchState::Running { pid } => (
            format!(
                "Oyun çalışıyor{}",
                pid.map(|p| format!(" (pid {p})")).unwrap_or_default()
            ),
            theme::success(),
        ),
        LaunchState::Failed(error) => (error.clone(), theme::danger()),
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new(8., 20., 8., 20.))
        .background(theme::surface())
        .child(
            label()
                .text(text)
                .font_size(12.)
                .max_lines(2)
                .color(color),
        )
}

// ---------------------------------------------------------------------------
// Hesap kartı — Microsoft device-code girişi
// ---------------------------------------------------------------------------

fn account_card(actions: &crate::actions::Actions) -> impl IntoElement {
    let login = use_radio(AppChannel::Account).read().login.clone();
    let settings = use_radio(AppChannel::Settings).read().settings.clone();

    let mut row: Vec<Element> = Vec::new();

    match login {
        LoginUiState::Waiting {
            user_code,
            verification_uri,
        } => {
            let uri = verification_uri.clone();
            let cancel_actions = actions.clone();
            row.push(
                label()
                    .text("Kod:")
                    .font_size(12.)
                    .color(theme::fg_muted())
                    .into_element(),
            );
            row.push(
                label()
                    .text(user_code)
                    .font_size(17.)
                    .font_weight(FontWeight::BOLD)
                    .color(theme::fg())
                    .into_element(),
            );
            row.push(
                label()
                    .text(verification_uri)
                    .font_size(11.)
                    .color(theme::fg_muted())
                    .into_element(),
            );
            row.push(
                DButton::new("Tarayıcıda aç")
                    .secondary()
                    .on_press(move |_| {
                        let _ = open::that_detached(&uri);
                    })
                    .into_element(),
            );
            row.push(
                DButton::new("Vazgeç")
                    .ghost()
                    .on_press(move |_| cancel_actions.cancel_login())
                    .into_element(),
            );
        }
        LoginUiState::Failed(error) => {
            let retry = actions.clone();
            row.push(
                label()
                    .text(format!("Giriş hatası: {error}"))
                    .font_size(12.)
                    .max_lines(2)
                    .color(theme::danger())
                    .into_element(),
            );
            row.push(
                DButton::new("Tekrar dene")
                    .on_press(move |_| retry.start_ms_login())
                    .into_element(),
            );
        }
        LoginUiState::None => {
            let account_line = match &settings.active {
                Some(account) => {
                    let kind = match &account.kind {
                        disco_auth::AccountKind::Microsoft { .. } => "Microsoft",
                        disco_auth::AccountKind::Offline => "çevrimdışı",
                    };
                    format!("Hesap: {} ({kind})", account.name)
                }
                None => format!("Hesap: {} (çevrimdışı)", settings.player_name),
            };
            let start = actions.clone();
            let offline = actions.clone();
            let ms_active = settings.active.is_some();

            row.push(
                label()
                    .text(account_line)
                    .font_size(12.)
                    .width(Size::fill())
                    .color(theme::fg())
                    .into_element(),
            );
            row.push(
                DButton::new("Microsoft ile giriş")
                    .on_press(move |_| start.start_ms_login())
                    .into_element(),
            );
            row.push(
                DButton::new("Çevrimdışı")
                    .secondary()
                    .enabled(!ms_active)
                    .on_press(move |_| offline.use_offline())
                    .into_element(),
            );
        }
    }

    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new(12., 16., 12., 16.))
        .spacing(10.)
        .cross_align(Alignment::Center)
        .corner_radius(CornerRadius::new_all(12.))
        .background(theme::surface())
        .overflow(Overflow::Clip)
        .children(row)
}

// ---------------------------------------------------------------------------
// Tema kartı — 8 renk vurgu seçici
// ---------------------------------------------------------------------------

fn theme_card(actions: &crate::actions::Actions) -> impl IntoElement {
    let settings = use_radio(AppChannel::Settings).read().settings.clone();

    let mut swatches: Vec<Element> = Vec::new();
    for hex in ACCENTS {
        let selected = settings.accent.eq_ignore_ascii_case(hex);
        let swatch_actions = actions.clone();
        swatches.push(
            rect()
                .width(Size::px(22.))
                .height(Size::px(22.))
                .corner_radius(CornerRadius::new_all(7.))
                .background(hex_to_color(hex))
                .border(
                    Border::new()
                        .fill(if selected { Color::WHITE } else { theme::surface_raised() })
                        .width(if selected { 2. } else { 1. })
                        .alignment(BorderAlignment::Inner),
                )
                .on_press(move |_| swatch_actions.set_accent(hex.to_string()))
                .cursor(CursorIcon::Pointer)
                .into_element(),
        );
    }

    rect()
        .vertical()
        .padding(Gaps::new(12., 16., 12., 16.))
        .spacing(10.)
        .corner_radius(CornerRadius::new_all(12.))
        .background(theme::surface())
        .child(
            label()
                .text("Vurgu rengi")
                .font_size(12.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(theme::fg()),
        )
        .child(
            rect()
                .horizontal()
                .spacing(8.)
                .children(swatches),
        )
}

// ---------------------------------------------------------------------------
// Mağaza — Modrinth
// ---------------------------------------------------------------------------

fn store_panel(store: &StoreState, actions: &crate::actions::Actions) -> impl IntoElement {
    let query = use_state(String::new);

    let mut rows: Vec<Element> = Vec::new();
    if store.loading {
        rows.push(
            label()
                .text("Aranıyor…")
                .font_size(12.)
                .color(theme::fg_muted())
                .into_element(),
        );
    } else if store.hits.is_empty() {
        rows.push(
            label()
                .text("Sonuç yok. Bir şeyler ara ya da \"Popüler\"e bas.")
                .font_size(12.)
                .color(theme::fg_muted())
                .into_element(),
        );
    } else {
        for hit in &store.hits {
            rows.push(store_row(hit, actions).into_element());
        }
    }

    let search_actions = actions.clone();
    let search_actions_btn = actions.clone();
    let popular_actions = actions.clone();
    let query_for_search = query.clone();

    rect()
        .vertical()
        .width(Size::fill())
        .height(Size::fill())
        .padding(Gaps::new(16., 16., 16., 16.))
        .spacing(12.)
        .corner_radius(CornerRadius::new_all(12.))
        .background(theme::surface())
        .overflow(Overflow::Clip)
        .child(
            label()
                .text("Modrinth Mağazası — Mod Paketleri")
                .font_size(15.)
                .font_weight(FontWeight::SEMI_BOLD)
                .color(theme::fg()),
        )
        .child(
            rect()
                .horizontal()
                .width(Size::fill())
                .spacing(8.)
                .cross_align(Alignment::Center)
                .child(
                    Input::new(query)
                        .placeholder("Mod paketi ara (ör. Simply Optimized, Fabulously Optimized)")
                        .padding(Gaps::new_symmetric(8., 12.))
                        .corner_radius(CornerRadius::new_all(8.))
                        .background(theme::surface_raised())
                        .border_fill(theme::bg())
                        .color(theme::fg())
                        .on_submit(move |text: String| {
                            search_actions.search_store(text);
                        }),
                )
                .child(
                    DButton::new("Ara")
                        .on_press(move |_| {
                            let value = (*query_for_search.read()).clone();
                            search_actions_btn.search_store(value);
                        })
                        .into_element(),
                )
                .child(
                    DButton::new("Popüler")
                        .secondary()
                        .on_press(move |_| popular_actions.show_popular())
                        .into_element(),
                ),
        )
        .children(rows)
}

fn store_row(hit: &StoreHit, actions: &crate::actions::Actions) -> impl IntoElement {
    let install = actions.clone();
    let project_id = hit.project_id.clone();
    let title = hit.title.clone();

    let downloads = if hit.downloads >= 1_000_000 {
        format!("{:.1}M", hit.downloads as f64 / 1_000_000.0)
    } else if hit.downloads >= 1_000 {
        format!("{:.1}K", hit.downloads as f64 / 1_000.0)
    } else {
        hit.downloads.to_string()
    };

    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new(10., 10., 10., 10.))
        .spacing(10.)
        .cross_align(Alignment::Center)
        .corner_radius(CornerRadius::new_all(10.))
        .background(theme::surface_raised())
        .child(
            rect()
                .vertical()
                .width(Size::fill())
                .spacing(2.)
                .child(
                    label()
                        .text(hit.title.clone())
                        .font_size(13.)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .color(theme::fg()),
                )
                .child(
                    label()
                        .text(hit.description.clone())
                        .font_size(11.)
                        .max_lines(2)
                        .color(theme::fg_muted()),
                ),
        )
        .child(
            label()
                .text(format!("{downloads} indirme · MC {}", hit.game_versions))
                .font_size(10.)
                .color(theme::fg_muted()),
        )
        .child(
            DButton::new("Kur")
                .on_press(move |_| {
                    install.install_modpack(project_id.clone(), title.clone());
                })
                .into_element(),
        )
}
