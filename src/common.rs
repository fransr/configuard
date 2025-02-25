use crate::config::config;
use crate::templates::BridgeRouterAliasTemplate;
use crate::templates::RouteAddTemplate;
use crate::templates::RouteDelTemplate;
use crate::templates::WireguardServerConfigurationEntryTemplate;
use crate::templates::WireguardServerConfigurationTemplate;
use crate::templates::WireguardSyncConfigTemplate;
use crate::templates::WireguardWorkstationTemplate;
use crate::utils::both_elements;
use crate::utils::first_of_pair;
use crate::utils::run;
use crate::ENTRIES_DIR;
use crate::SERVER_PRIVATE_KEY;
use crate::SERVER_PUBLIC_KEY;
use askama::Template;
use rand_core::OsRng;
use std::ffi::OsStr;
use std::fs::read_to_string;
use walkdir::{DirEntry, WalkDir};
use x25519_dalek::{PublicKey, StaticSecret};


pub fn generate_wireguard_keys() -> (String, String) {
    let private = StaticSecret::new(&mut OsRng);
    let public = PublicKey::from(&private);
    let public_b64 = base64::encode(public.as_bytes());
    let private_b64 = base64::encode(&private.to_bytes());
    (private_b64, public_b64)
}


pub fn commit_wireguard_configuration(user_ipv4: &str) {
    // NOTE: Create bridge0 with router ip assigned to it. Don't assign .1.1 to server-side wg
    // println!("Setting up bridge0");
    run(
        "/dev/stderr",
        BridgeRouterAliasTemplate {
            router_ip_address: &format!("{}.1.1", config().main_net),
            net_mask: &config().main_net_mask,
        },
    )
    .ok();

    // for ipv6: route -6 add fde4:82c4:04eb:dd8d::1:5 -interface wg0
    // println!("Setting up Wireguard routes for: {}", &user_ipv4);
    run(
        "/dev/stderr",
        RouteDelTemplate {
            ipv4_address: &user_ipv4,
        },
    )
    .ok();

    run(
        "/dev/stderr",
        RouteAddTemplate {
            ipv4_address: &user_ipv4,
        },
    )
    .ok();

    // println!("Synchronizing server configuration");
    run(
        "/dev/stdout",
        WireguardSyncConfigTemplate {
            wireguard_bin: &config().wireguard_bin,
            wireguard_conf: &config().wireguard_conf,
        },
    )
    .ok();
}


pub fn render_all_entries() -> String {
    let all_entries = read_files_list(ENTRIES_DIR);
    let all_entries_ipv4s_and_pubkeys = all_entries
        .iter()
        .filter_map(|file| read_to_string(file.path()).ok())
        .filter_map(|line| both_elements(&line))
        .collect::<Vec<_>>();

    all_entries
        .iter()
        .zip(&all_entries_ipv4s_and_pubkeys)
        .map(|(config_name, (ip, pubkey))| {
            // entries
            format!(
                "{}\n\n",
                (WireguardServerConfigurationEntryTemplate {
                    user_name: &file_name_to_string(config_name.file_name()),
                    user_ips: &ip,
                    user_public_key: pubkey,
                })
                .render()
                .unwrap_or_default()
            )
        })
        .collect::<String>()
}


pub fn render_server_config_head() -> String {
    (WireguardServerConfigurationTemplate {
        server_port: &format!("{}", config().server_port),
        server_private_key: &read_server_key(SERVER_PRIVATE_KEY),
    })
    .render()
    .unwrap_or_default()
}


pub fn is_not_hidden_file(file: &DirEntry) -> bool {
    file.path().is_file() && !file_name_to_string(file.file_name()).starts_with('.')
}


pub fn read_files_list(from_subdir: &str) -> Vec<DirEntry> {
    WalkDir::new(from_subdir)
        .into_iter()
        .filter_map(|v| v.ok())
        .filter(is_not_hidden_file)
        .collect()
}


pub fn file_name_to_string(name: &OsStr) -> String {
    name.to_os_string().into_string().unwrap_or_default()
}


pub fn read_all_used_ipv4(from_subdir: &str) -> Vec<String> {
    WalkDir::new(&format!("{}{}", ENTRIES_DIR, from_subdir))
        .into_iter()
        .filter_map(|v| v.ok())
        .filter(is_not_hidden_file)
        .filter_map(|file| read_to_string(file.path()).ok())
        .filter_map(|line| first_of_pair(&line))
        .collect()
}


pub fn read_server_key(file: &str) -> String {
    read_to_string(file).unwrap_or_default().replace('\n', "")
}


pub fn random_name(length: usize) -> String {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};
    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .collect()
}


pub fn random_byte() -> u8 {
    use rand::{thread_rng, Rng};
    let mut rng = thread_rng();
    rng.gen_range(2, 254)
}


pub fn random_word() -> u16 {
    use rand::{thread_rng, Rng};
    let mut rng = thread_rng();
    rng.gen_range(966, 65535)
}


pub fn new_decoy() -> String {
    let (private_key, _) = generate_wireguard_keys();
    let user_ipv4 = format!(
        "{}.{}.{}.{}",
        random_byte(),
        random_byte(),
        random_byte(),
        random_byte()
    );
    let user_nets = format!("{}/{}", user_ipv4, random_byte() % 32 + 9);
    let user_template = WireguardWorkstationTemplate {
        user_name: &random_name(10),
        user_private_key: &private_key,
        user_nets: &user_nets,
        server_public_key: &read_server_key(SERVER_PUBLIC_KEY),
        default_server_endpoint: &format!("{}:{}", config().server_public_ip, &random_word()),
    };

    format!("{}\n", user_template.render().unwrap_or_default())
}
