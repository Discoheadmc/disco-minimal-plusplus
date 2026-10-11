//! Ana ekran.

use freya::prelude::*;
use freya::radio::use_radio;

use crate::actions::use_dispatch;
use crate::components::{ButtonVariant, DButton};
use crate::state::{AppChannel, LaunchState, ManifestState};
use crate::theme;
use disco_core::Instance;

#[derive(PartialEq)]
pub struct Home;

impl Component for Home {
    fn render(&self) -> impl IntoElement {
        let actions = use_dispatch();
        let instances = use_radio(AppChannel::Instances).read().instances.clone();
        let manifest = use_radio(AppChannel::Manifest).read().manifest.clone();
        let launch = use_radio(AppChannel::Launch).read().launch.clone();
        let player = use_radio(AppChannel::Settings).read().settings.player_name.clone();

        let busy = !matches!(launch, LaunchState::Idle | LaunchState::Failed(_));

        rect()
            .vertical()
            .width(Size::fill())
            .height(Size::fill())
            .background(theme::bg())
            .child(header())
            .child(
                rect()
                    .horizontal()
                    .width(Size::fill())
                    .height(Size::fill())
                    .padding(Gaps::new(20., 20., 20., 20.))
                    .spacing(16.)
                    .child(instances_panel(&instances, busy, &actions))
                    .child(create_panel(&manifest, &player, busy, &actions)),
            )
            .child(status_bar(&launch))
    }
}

fn header() -> impl IntoElement {
    rect()
        .horizontal()
        .width(Size::fill())
        .padding(Gaps::new(12., 20., 12., 20.))
        .spacing(12.)
        .cross_align(Alignment::Center)
        .background(theme::surface())
        .child(
            rect()
                .width(Size::px(34.))
                .height(Size::px(34.))
                .corner_radius(CornerRadius::new_all(10.))
                .background(theme::accent())
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
            rect()
                .width(Size::fill())
                .main_align(Alignment::End)
                .child(
                    label()
                        .text(format!("v{} — Rust + Freya", disco_common::constants::VERSION))
                        .font_size(12.)
                        .color(theme::fg_muted()),
                ),
        )
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
