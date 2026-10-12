//! Minimal buton — OneLauncher'ın Button'ından ilhamla, ihtiyacımız kadarı.

use freya::prelude::*;

use crate::theme;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Selected,
}

#[derive(Clone, PartialEq)]
pub struct DButton {
    text: String,
    variant: ButtonVariant,
    enabled: bool,
    on_press: Option<EventHandler<Event<PressEventData>>>,
}

impl DButton {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            variant: ButtonVariant::default(),
            enabled: true,
            on_press: None,
        }
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    #[allow(dead_code)] // Kilometre Taşı 1'de ayarlar/iptal butonlarında kullanılacak
    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn on_press(mut self, on_press: impl Into<EventHandler<Event<PressEventData>>>) -> Self {
        self.on_press = Some(on_press.into());
        self
    }
}

impl Component for DButton {
    fn render(&self) -> impl IntoElement {
        let mut hovering = use_state(|| false);
        let mut pressing = use_state(|| false);

        let enabled = self.enabled;
        let accent = crate::components::use_accent();

        let (base, hover, pressed) = match self.variant {
            ButtonVariant::Primary | ButtonVariant::Selected => {
                (accent.base, accent.hover, accent.pressed)
            }
            ButtonVariant::Secondary => (
                theme::surface_raised(),
                theme::surface_pressed(),
                theme::bg(),
            ),
            ButtonVariant::Ghost => (Color::TRANSPARENT, theme::surface_raised(), theme::bg()),
        };

        let background = if !enabled {
            base
        } else if pressing() {
            pressed
        } else if hovering() {
            hover
        } else {
            base
        };

        let foreground = match self.variant {
            ButtonVariant::Secondary | ButtonVariant::Ghost if !enabled => theme::fg_muted(),
            ButtonVariant::Secondary | ButtonVariant::Ghost => theme::fg(),
            _ if !enabled => theme::fg_muted(),
            _ => Color::WHITE,
        };

        let mut on_press = self.on_press.clone();

        let mut rect = rect()
            .horizontal()
            .main_align(Alignment::Center)
            .cross_align(Alignment::Center)
            .padding(Gaps::new_symmetric(8., 16.))
            .corner_radius(CornerRadius::new_all(8.))
            .background(background)
            .child(
                label()
                    .text(self.text.clone())
                    .font_size(14.)
                    .font_weight(FontWeight::MEDIUM)
                    .color(foreground),
            );

        if enabled {
            if let Some(handler) = on_press.take() {
                rect = rect.on_press(move |event: Event<PressEventData>| {
                    handler.call(event);
                });
            }
            rect = rect
                .on_pointer_down(move |_| pressing.set(true))
                .on_global_pointer_up(move |_| pressing.set(false))
                .on_pointer_over(move |_| hovering.set(true))
                .on_pointer_out(move |_| {
                    hovering.set(false);
                    pressing.set(false);
                })
                .cursor(CursorIcon::Pointer);
        } else {
            rect = rect.opacity(0.5).cursor(CursorIcon::NotAllowed);
        }

        rect
    }
}
