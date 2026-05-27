//! W13.A: wp-color-management-v1 protocol (sRGB-only, Galaxy Book 4 SDR panel).
//!
//! Ref: https://wayland.app/protocols/color-management-v1 (Feb 2025 staging).
//! Mesa MR 31991: Vulkan WSI client-side support.
//!
//! Politica: Galaxy Book 4 painel 6-bit + FRC = SDR fixo.
//!   - sRGB sempre aceito (advertised ao bind).
//!   - Display-P3, BT.2020/PQ: parametric aceito e ignorado; ICC rejeitado com failed.
//!   - Pos-Wave 13: expandir quando HDMI external HDR display.
//!
//! Implementacao server-side manual (smithay 0.7 nao tem helper pra
//! color-management; somente fifo e commit_timing foram integrados).
//! Seguimos o mesmo padrao de screencopy.rs: Dispatch manual + GlobalDispatch.

use smithay::reexports::wayland_protocols::wp::color_management::v1::server::{
    wp_color_management_output_v1::{self, WpColorManagementOutputV1},
    wp_color_management_surface_feedback_v1::{self, WpColorManagementSurfaceFeedbackV1},
    wp_color_management_surface_v1::{self, WpColorManagementSurfaceV1},
    wp_color_manager_v1::{self, WpColorManagerV1},
    wp_image_description_creator_icc_v1::{self, WpImageDescriptionCreatorIccV1},
    wp_image_description_creator_params_v1::{self, WpImageDescriptionCreatorParamsV1},
    wp_image_description_info_v1::WpImageDescriptionInfoV1,
    wp_image_description_v1::{self, WpImageDescriptionV1},
};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New,
};

use crate::state::LumoState;

/// W37.12: gerador de image_description IDs unicos.
/// Spec wp-color-management-v1: "Zero is reserved as an invalid id number.
/// A compositor shall not send an invalid id number." Antes enviavamos
/// `ready(0)` -> Chromium fechava conexao com broken pipe pos handshake.
fn next_image_description_id() -> u32 {
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Gerencia o global wp_color_manager_v1.
pub struct ColorManagerState {
    pub global: smithay::reexports::wayland_server::backend::GlobalId,
}

impl ColorManagerState {
    pub fn new(display: &DisplayHandle) -> Self {
        let global = display.create_global::<LumoState, WpColorManagerV1, _>(1, ());
        ColorManagerState { global }
    }
}

impl GlobalDispatch<WpColorManagerV1, ()> for LumoState {
    fn bind(
        _state: &mut LumoState,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<WpColorManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, LumoState>,
    ) {
        use smithay::reexports::wayland_protocols::wp::color_management::v1::server::wp_color_manager_v1::{
            RenderIntent, Feature, TransferFunction, Primaries,
        };
        let manager = data_init.init(resource, ());
        manager.supported_intent(RenderIntent::Perceptual);
        manager.supported_feature(Feature::Parametric);
        manager.supported_tf_named(TransferFunction::Srgb);
        manager.supported_primaries_named(Primaries::Srgb);
        manager.done();
        tracing::debug!("W13.A: wp_color_manager_v1 bound, advertised sRGB-only capabilities");
    }
}

impl Dispatch<WpColorManagerV1, ()> for LumoState {
    fn request(
        _state: &mut LumoState,
        _client: &Client,
        _manager: &WpColorManagerV1,
        request: wp_color_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, LumoState>,
    ) {
        match request {
            wp_color_manager_v1::Request::GetOutput { id, output: _ } => {
                data_init.init(id, ());
            }
            wp_color_manager_v1::Request::GetSurface { id, surface: _ } => {
                data_init.init(id, ());
            }
            wp_color_manager_v1::Request::GetSurfaceFeedback { id, surface: _ } => {
                data_init.init(id, ());
            }
            wp_color_manager_v1::Request::CreateParametricCreator { obj } => {
                data_init.init(obj, ());
            }
            wp_color_manager_v1::Request::CreateIccCreator { obj } => {
                data_init.init(obj, ());
            }
            wp_color_manager_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("W13.A: wp_color_manager_v1: unhandled request");
            }
        }
    }
}

impl Dispatch<WpColorManagementOutputV1, ()> for LumoState {
    fn request(
        _state: &mut LumoState,
        _client: &Client,
        _obj: &WpColorManagementOutputV1,
        request: wp_color_management_output_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, LumoState>,
    ) {
        match request {
            wp_color_management_output_v1::Request::GetImageDescription { image_description } => {
                let desc = data_init.init(image_description, ImageDescriptionKind::Srgb);
                desc.ready(next_image_description_id());
                tracing::debug!("W13.A: output get_image_description -> sRGB ready");
            }
            wp_color_management_output_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<WpColorManagementSurfaceV1, ()> for LumoState {
    fn request(
        _state: &mut LumoState,
        _client: &Client,
        _obj: &WpColorManagementSurfaceV1,
        request: wp_color_management_surface_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, LumoState>,
    ) {
        match request {
            wp_color_management_surface_v1::Request::SetImageDescription {
                image_description,
                render_intent: _,
            } => {
                let _ = image_description;
                tracing::debug!("W13.A: surface set_image_description accepted (Galaxy SDR)");
            }
            wp_color_management_surface_v1::Request::UnsetImageDescription => {
                tracing::debug!("W13.A: surface unset_image_description");
            }
            wp_color_management_surface_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<WpColorManagementSurfaceFeedbackV1, ()> for LumoState {
    fn request(
        _state: &mut LumoState,
        _client: &Client,
        _obj: &WpColorManagementSurfaceFeedbackV1,
        request: wp_color_management_surface_feedback_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, LumoState>,
    ) {
        match request {
            wp_color_management_surface_feedback_v1::Request::GetPreferred {
                image_description,
            } => {
                let desc = data_init.init(image_description, ImageDescriptionKind::Srgb);
                desc.ready(next_image_description_id());
                tracing::debug!("W13.A: surface_feedback get_preferred -> sRGB");
            }
            wp_color_management_surface_feedback_v1::Request::GetPreferredParametric {
                image_description,
            } => {
                let desc = data_init.init(image_description, ImageDescriptionKind::Srgb);
                desc.ready(next_image_description_id());
            }
            wp_color_management_surface_feedback_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

/// Tipo de descricao de imagem retornada ao cliente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageDescriptionKind {
    /// sRGB, padrao Galaxy Book 4 SDR panel.
    Srgb,
    /// Criada por cliente via parametric creator; tratada como sRGB.
    ClientParametric,
    /// Rejeitada (ICC pedido, nao suportado neste display).
    NotSupported,
}

impl Dispatch<WpImageDescriptionV1, ImageDescriptionKind> for LumoState {
    fn request(
        _state: &mut LumoState,
        _client: &Client,
        _obj: &WpImageDescriptionV1,
        request: wp_image_description_v1::Request,
        _data: &ImageDescriptionKind,
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, LumoState>,
    ) {
        match request {
            wp_image_description_v1::Request::Destroy => {}
            wp_image_description_v1::Request::GetInformation { information } => {
                eprintln!("[wm] W37.13 get_information chamado");
                let info = data_init.init(information, ());
                eprintln!("[wm] W37.13 info inicializado");
                use smithay::reexports::wayland_protocols::wp::color_management::v1::server::wp_color_manager_v1::{Primaries, TransferFunction};
                // sRGB primaries (BT.709) coords * 1e6
                info.primaries(640_000, 330_000, 300_000, 600_000, 150_000, 60_000, 312_700, 329_000);
                info.primaries_named(Primaries::Srgb);
                info.tf_named(TransferFunction::Srgb);
                // Min luminance 0.2 cd/m2 (* 10000), max 80 cd/m2, ref white 80.
                info.luminances(2_000, 80, 80);
                // W37.13: spec exige target_primaries MANDATORY pra parametric.
                // Antes omitido por leitura errada da spec -> Chromium fechava
                // conexao com Broken pipe apos get_information.
                info.target_primaries(640_000, 330_000, 300_000, 600_000, 150_000, 60_000, 312_700, 329_000);
                info.target_luminance(0, 80);
                info.done();
                eprintln!("[wm] W37.13 get_information -> done() enviado");
            }
            _ => {}
        }
    }
}

impl Dispatch<WpImageDescriptionInfoV1, ()> for LumoState {
    fn request(
        _state: &mut LumoState,
        _client: &Client,
        _obj: &WpImageDescriptionInfoV1,
        _request: smithay::reexports::wayland_protocols::wp::color_management::v1::server::wp_image_description_info_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, LumoState>,
    ) {
        // info_v1 nao tem requests (so eventos). Stub.
    }
}

impl Dispatch<WpImageDescriptionCreatorParamsV1, ()> for LumoState {
    fn request(
        _state: &mut LumoState,
        _client: &Client,
        _obj: &WpImageDescriptionCreatorParamsV1,
        request: wp_image_description_creator_params_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, LumoState>,
    ) {
        match request {
            wp_image_description_creator_params_v1::Request::Create { image_description } => {
                let desc =
                    data_init.init(image_description, ImageDescriptionKind::ClientParametric);
                desc.ready(next_image_description_id());
                tracing::debug!("W13.A: parametric creator: create -> ClientParametric ready");
            }
            wp_image_description_creator_params_v1::Request::SetTfNamed { .. }
            | wp_image_description_creator_params_v1::Request::SetTfPower { .. }
            | wp_image_description_creator_params_v1::Request::SetPrimariesNamed { .. }
            | wp_image_description_creator_params_v1::Request::SetPrimaries { .. }
            | wp_image_description_creator_params_v1::Request::SetLuminances { .. }
            | wp_image_description_creator_params_v1::Request::SetMasteringDisplayPrimaries {
                ..
            }
            | wp_image_description_creator_params_v1::Request::SetMasteringLuminance { .. }
            | wp_image_description_creator_params_v1::Request::SetMaxCll { .. }
            | wp_image_description_creator_params_v1::Request::SetMaxFall { .. } => {
                tracing::debug!("W13.A: parametric parameter accepted (noop SDR)");
            }
            _ => {}
        }
    }
}

impl Dispatch<WpImageDescriptionCreatorIccV1, ()> for LumoState {
    fn request(
        _state: &mut LumoState,
        _client: &Client,
        _obj: &WpImageDescriptionCreatorIccV1,
        request: wp_image_description_creator_icc_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, LumoState>,
    ) {
        match request {
            wp_image_description_creator_icc_v1::Request::Create { image_description } => {
                let desc = data_init.init(image_description, ImageDescriptionKind::NotSupported);
                desc.failed(
                    wp_image_description_v1::Cause::Unsupported,
                    "ICC profiles not supported on Galaxy Book 4 SDR panel".to_string(),
                );
                tracing::debug!("W13.A: icc_creator: create -> failed (ICC nao suportado)");
            }
            wp_image_description_creator_icc_v1::Request::SetIccFile { .. } => {}
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_description_kind_srgb_eq_self() {
        assert_eq!(ImageDescriptionKind::Srgb, ImageDescriptionKind::Srgb);
    }

    #[test]
    fn image_description_kind_not_supported_differs_from_srgb() {
        assert_ne!(
            ImageDescriptionKind::Srgb,
            ImageDescriptionKind::NotSupported
        );
    }

    #[test]
    fn image_description_kind_client_parametric_is_distinct() {
        let kind = ImageDescriptionKind::ClientParametric;
        assert_ne!(kind, ImageDescriptionKind::Srgb);
        assert_ne!(kind, ImageDescriptionKind::NotSupported);
    }

    #[test]
    fn image_description_kind_debug_contains_variant_name() {
        let s = format!("{:?}", ImageDescriptionKind::Srgb);
        assert!(s.contains("Srgb"));
        let s2 = format!("{:?}", ImageDescriptionKind::NotSupported);
        assert!(s2.contains("NotSupported"));
    }

    #[test]
    fn image_description_kind_clone_eq() {
        let a = ImageDescriptionKind::ClientParametric;
        let b = a;
        assert_eq!(a, b);
    }
}
