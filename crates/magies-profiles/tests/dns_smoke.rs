use std::fs;
use std::path::PathBuf;
use std::process::Command;

use magies_profiles::{
    DnsProfile, DnsServer, DnsStrategy, PlainDnsTransport, SingBoxDnsConfigGenerator,
};
use serde_json::json;

#[test]
#[ignore = "requires MAGIES_SING_BOX_DNS_BIN pointing to official sing-box 1.13.18"]
fn official_sing_box_accepts_generated_dns_config() {
    let binary = PathBuf::from(
        std::env::var_os("MAGIES_SING_BOX_DNS_BIN")
            .expect("MAGIES_SING_BOX_DNS_BIN must point to official sing-box"),
    );
    let profile = DnsProfile::new(
        vec![
            DnsServer::plain("bootstrap", PlainDnsTransport::Udp, "1.1.1.1", 53, None).unwrap(),
            DnsServer::doh("secure", "dns.google", 443, "/dns-query", "bootstrap").unwrap(),
        ],
        Vec::new(),
        "secure",
        DnsStrategy::PreferIpv4,
        true,
        true,
    )
    .unwrap();
    let dns = SingBoxDnsConfigGenerator::generate(&profile);
    let config = json!({
        "log": { "level": "warn" },
        "dns": dns.json(),
        "inbounds": [{
            "type": "socks",
            "tag": "socks-in",
            "listen": "127.0.0.1",
            "listen_port": 10808
        }],
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": { "default_domain_resolver": "secure", "final": "direct" }
    });
    let config_path = std::env::temp_dir().join(format!(
        "magies-dns-smoke-{}-{}.json",
        std::process::id(),
        std::env::consts::OS
    ));
    fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();

    let output = Command::new(binary)
        .arg("check")
        .arg("-c")
        .arg(&config_path)
        .output()
        .unwrap();
    fs::remove_file(config_path).unwrap();

    assert!(
        output.status.success(),
        "sing-box rejected generated DNS config: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
