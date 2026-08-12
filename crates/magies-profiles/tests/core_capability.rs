//! Covers the Core capability matrix and the selection policy from PRD 14:
//! what each Core can serve, why one gets ruled out, and which one Auto picks.

use magies_domain::{CoreType, ProxyProtocol};
use magies_platform::CpuArchitecture;
use magies_profiles::{
    CoreCapabilityMatrix, CorePreference, CoreRejection, CoreRequirements, CoreSelectionError,
    core_name, parse_core_name,
};

fn requirements(protocol: ProxyProtocol, tun: bool) -> CoreRequirements {
    CoreRequirements::new(protocol, tun, CpuArchitecture::Aarch64)
}

const STREAM_PROTOCOLS: [ProxyProtocol; 4] = [
    ProxyProtocol::Vless,
    ProxyProtocol::Vmess,
    ProxyProtocol::Trojan,
    ProxyProtocol::Shadowsocks,
];

#[test]
fn sing_box_serves_every_v01_protocol() {
    for protocol in [
        ProxyProtocol::Vless,
        ProxyProtocol::Vmess,
        ProxyProtocol::Trojan,
        ProxyProtocol::Shadowsocks,
        ProxyProtocol::Hysteria2,
    ] {
        assert!(
            CoreCapabilityMatrix::supports(CoreType::SingBox, requirements(protocol, false)),
            "sing-box should support {protocol:?}"
        );
    }
}

#[test]
fn xray_serves_the_stream_protocols_but_not_hysteria2() {
    for protocol in STREAM_PROTOCOLS {
        assert!(
            CoreCapabilityMatrix::supports(CoreType::Xray, requirements(protocol, false)),
            "xray should support {protocol:?}"
        );
    }

    assert_eq!(
        CoreCapabilityMatrix::rejection(
            CoreType::Xray,
            requirements(ProxyProtocol::Hysteria2, false)
        ),
        Some(CoreRejection::ProtocolUnsupported {
            core: CoreType::Xray,
            protocol: ProxyProtocol::Hysteria2,
        })
    );
}

#[test]
fn only_sing_box_provides_tun() {
    assert!(CoreCapabilityMatrix::supports(
        CoreType::SingBox,
        requirements(ProxyProtocol::Vless, true)
    ));

    assert_eq!(
        CoreCapabilityMatrix::rejection(CoreType::Xray, requirements(ProxyProtocol::Vless, true)),
        Some(CoreRejection::TunUnsupported {
            core: CoreType::Xray
        })
    );
}

#[test]
fn both_cores_build_for_both_architectures() {
    for architecture in [CpuArchitecture::X86_64, CpuArchitecture::Aarch64] {
        for core in [CoreType::SingBox, CoreType::Xray] {
            assert!(CoreCapabilityMatrix::supports(
                core,
                CoreRequirements::new(ProxyProtocol::Vless, false, architecture)
            ));
        }
    }
}

#[test]
fn auto_prefers_sing_box_for_the_general_case() {
    for protocol in STREAM_PROTOCOLS {
        assert_eq!(
            CoreCapabilityMatrix::select(CorePreference::Auto, requirements(protocol, false)),
            Ok(CoreType::SingBox)
        );
    }
}

#[test]
fn auto_keeps_sing_box_for_hysteria2_and_for_tun() {
    assert_eq!(
        CoreCapabilityMatrix::select(
            CorePreference::Auto,
            requirements(ProxyProtocol::Hysteria2, false)
        ),
        Ok(CoreType::SingBox)
    );
    assert_eq!(
        CoreCapabilityMatrix::select(
            CorePreference::Auto,
            requirements(ProxyProtocol::Vless, true)
        ),
        Ok(CoreType::SingBox)
    );
}

#[test]
fn an_explicit_choice_is_honoured() {
    assert_eq!(
        CoreCapabilityMatrix::select(
            CorePreference::Fixed(CoreType::Xray),
            requirements(ProxyProtocol::Vless, false)
        ),
        Ok(CoreType::Xray)
    );
    assert_eq!(
        CoreCapabilityMatrix::select(
            CorePreference::Fixed(CoreType::SingBox),
            requirements(ProxyProtocol::Vless, false)
        ),
        Ok(CoreType::SingBox)
    );
}

#[test]
fn an_impossible_explicit_choice_is_an_error_rather_than_a_substitution() {
    // Silently falling back would start a session on a Core the user did not
    // pick, which is worse than telling them why their choice cannot work.
    let error = CoreCapabilityMatrix::select(
        CorePreference::Fixed(CoreType::Xray),
        requirements(ProxyProtocol::Hysteria2, false),
    )
    .unwrap_err();

    assert_eq!(
        error,
        CoreSelectionError::ChosenCoreUnusable {
            rejection: CoreRejection::ProtocolUnsupported {
                core: CoreType::Xray,
                protocol: ProxyProtocol::Hysteria2,
            }
        }
    );
    assert_eq!(
        error.to_string(),
        "the selected Core cannot run this node: xray does not support Hysteria2"
    );
}

#[test]
fn choosing_xray_with_tun_explains_the_conflict() {
    let error = CoreCapabilityMatrix::select(
        CorePreference::Fixed(CoreType::Xray),
        requirements(ProxyProtocol::Vless, true),
    )
    .unwrap_err();

    assert_eq!(
        error.to_string(),
        "the selected Core cannot run this node: xray cannot provide TUN mode"
    );
}

#[test]
fn auto_is_the_default_preference() {
    assert_eq!(CorePreference::default(), CorePreference::Auto);
}

#[test]
fn core_names_are_stable_in_both_directions() {
    for core in [CoreType::SingBox, CoreType::Xray] {
        assert_eq!(parse_core_name(core_name(core)), Some(core));
    }
    assert_eq!(parse_core_name("clash"), None);
}

/// Both rejection kinds below are unreachable with today's matrix — sing-box
/// serves every protocol and both Cores ship for both architectures — so their
/// messages are pinned directly, ready for when the matrix grows.
#[test]
fn the_unreachable_rejections_still_read_correctly() {
    assert_eq!(
        CoreRejection::ArchitectureUnsupported {
            core: CoreType::Xray,
            architecture: CpuArchitecture::Aarch64,
        }
        .to_string(),
        "xray has no build for aarch64"
    );
    assert_eq!(
        CoreSelectionError::NoUsableCore {
            protocol: ProxyProtocol::Hysteria2,
            tun: true,
        }
        .to_string(),
        "no available Core supports Hysteria2 with TUN on"
    );
    assert_eq!(
        CoreSelectionError::NoUsableCore {
            protocol: ProxyProtocol::Vless,
            tun: false,
        }
        .to_string(),
        "no available Core supports VLESS with TUN off"
    );
}

#[test]
fn every_capability_entry_reports_the_shared_abilities() {
    for core in [CoreType::SingBox, CoreType::Xray] {
        for capability in CoreCapabilityMatrix::capabilities(core) {
            assert!(capability.supports_udp);
            assert!(capability.supports_reality);
            assert!(capability.supports_mux);
            assert_eq!(capability.architectures.len(), 2);
        }
    }
}
