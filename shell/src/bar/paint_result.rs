
use std::time::Instant;
/// Resultado de paint_frame: posicoes calculadas pra hit-test no proximo frame.
#[derive(Default, Clone)]
pub(crate) struct PaintResult {
    pub bat_hit_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_hit_rect: Option<(f32, f32, f32, f32)>,     // A23
    pub datetime_hit_rect: Option<(f32, f32, f32, f32)>, // A24
    pub lumo_hit_rect: Option<(f32, f32, f32, f32)>,     // A27: brand "Lumo" pill esquerda
    // A26: hit-tests do calendar interativo (so populados quando dropdown=DateTime).
    pub cal_prev_rect: Option<(f32, f32, f32, f32)>,
    pub cal_next_rect: Option<(f32, f32, f32, f32)>,
    pub cal_today_rect: Option<(f32, f32, f32, f32)>,
    /// Cada (day, rect). Day = dia do mes visualizado (1..=31), rect em coords surface.
    pub cal_day_rects: Vec<(u32, (f32, f32, f32, f32))>,
    // S2: appmenu fallback hit-rects pill + dropdown.
    pub appmenu_fallback_rect: Option<(f32, f32, f32, f32)>,
    pub appmenu_fallback_dropdown_rects: Vec<(usize, (f32, f32, f32, f32))>,
    // L5: brightness pill hit-rect.
    pub brightness_hit_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_slider_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_day_rect: Option<(f32, f32, f32, f32)>,
    pub brightness_preset_night_rect: Option<(f32, f32, f32, f32)>,
    // L5: battery dropdown interactive hit-rects.
    pub bat_charge_limit_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub bat_profile_cycle_rect: Option<(f32, f32, f32, f32)>,
    // A31.2: hit-rects do dropdown wifi (so populados quando dropdown=Wifi).
    pub wifi_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_disconnect_rect: Option<(f32, f32, f32, f32)>,
    pub wifi_connect_rects: Vec<(String, (f32, f32, f32, f32))>,
    
    // A31.6: Campos novos pro layout dinamico
    pub dropdown_h: f32,
    pub dropdown_rect_real: Option<(f32, f32, f32, f32)>,
    
    pub last_click_at: Option<Instant>,
    // A31.3: hit-rects do modal de senha wifi.
    pub pwd_confirm_rect: Option<(f32, f32, f32, f32)>,
    pub pwd_cancel_rect: Option<(f32, f32, f32, f32)>,
    // C5: hit-rects pills appmenu top-level (idx, rect).
    pub appmenu_pill_rects: Vec<(usize, (f32, f32, f32, f32))>,
    // C5: hit-rects subitens submenu aberto (sidx, rect).
    pub appmenu_submenu_rects: Vec<(usize, (f32, f32, f32, f32))>,
}
