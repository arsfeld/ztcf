extern crate dotenv;

use async_trait::async_trait;
use dotenv::dotenv;
use reqwest::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::net::Ipv4Addr;
use tokio::time::{sleep, Duration};

use cloudflare::endpoints::{dns, zone};
use cloudflare::framework::{
    async_api::ApiClient,
    async_api::Client,
    auth::Credentials,
    response::{ApiFailure, ApiResponse, ApiResult},
    Environment, HttpApiClientConfig, OrderDirection,
};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct IpAssignmentPool {
    ip_range_start: Ipv4Addr,
    ip_range_end: Ipv4Addr,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Route {
    target: String,
    via: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct NetworkConfig {
    ip_assignment_pools: Vec<IpAssignmentPool>,
    routes: Vec<Route>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Network {
    id: String,
    config: NetworkConfig,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct NetworkMemberConfig {
    ip_assignments: Vec<Ipv4Addr>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct NetworkMember {
    id: String,
    clock: usize,
    network_id: String,
    node_id: String,
    controller_id: String,
    config: NetworkMemberConfig,
    hidden: bool,
    name: String,
    description: String,
    last_online: usize,
    physical_address: String,
    client_version: String,
    protocol_version: u32,
    supports_rules_engine: bool,
}

async fn get_zt_network() -> Result<Network, Error> {
    let client = reqwest::Client::new();

    let request_url = format!(
        "https://my.zerotier.com/api/network/{network_id}",
        network_id = env::var("ZT_NETWORK_ID").unwrap()
    );
    println!("{}", request_url);
    let response = client
        .get(&request_url)
        .header(
            "Authorization",
            format!("Bearer {}", env::var("ZT_API_TOKEN").unwrap()),
        )
        .send()
        .await?;

    let network: Network = response.json().await?;
    println!("{:?}", network);

    Ok(network)
}

async fn get_zt_ips() -> Result<HashMap<String, Ipv4Addr>, Error> {
    let client = reqwest::Client::new();

    let request_url = format!(
        "https://my.zerotier.com/api/network/{network_id}/member",
        network_id = env::var("ZT_NETWORK_ID").unwrap()
    );
    println!("{}", request_url);
    let response = client
        .get(&request_url)
        .header(
            "Authorization",
            format!("Bearer {}", env::var("ZT_API_TOKEN").unwrap()),
        )
        .send()
        .await?;

    let members: Vec<NetworkMember> = response.json().await?;
    let ips: HashMap<String, Ipv4Addr> = members
        .into_iter()
        .map(|x| (x.name, x.config.ip_assignments[0]))
        .collect();
    //println!("{:?}", ips);

    Ok(ips)
}

#[async_trait]
trait DNS {
    async fn get_records(&self) -> Result<HashMap<String, String>, Error>;
}

struct CloudflareDNS {
    client: Client,
    zone_id: String,
}

fn print_response<T: ApiResult>(response: ApiResponse<T>) {
    match response {
        Ok(success) => println!("Success: {:#?}", success),
        Err(e) => match e {
            ApiFailure::Error(status, errors) => {
                println!("HTTP {}:", status);
                for err in errors.errors {
                    println!("Error {}: {}", err.code, err.message);
                    for (k, v) in err.other {
                        println!("{}: {}", k, v);
                    }
                }
                for (k, v) in errors.other {
                    println!("{}: {}", k, v);
                }
            }
            ApiFailure::Invalid(reqwest_err) => println!("Error: {}", reqwest_err),
        },
    }
}

impl CloudflareDNS {
    fn new(zone_id: String) -> CloudflareDNS {
        CloudflareDNS {
            zone_id,
            client: Client::new(
                Credentials::UserAuthToken {
                    token: env::var("CF_TOKEN").unwrap(),
                },
                HttpApiClientConfig::default(),
                Environment::Production,
            )
            .unwrap(),
        }
    }

    async fn get_records(&self) -> Result<HashMap<String, dns::DnsRecord>, Error> {
        let zone_details = self
            .client
            .request(&zone::ZoneDetails {
                identifier: &self.zone_id,
            })
            .await
            .unwrap()
            .result;

        let zone_name = zone_details.name;

        let existing_records = self
            .client
            .request(&dns::ListDnsRecords {
                zone_identifier: &self.zone_id,
                params: dns::ListDnsRecordsParams {
                    direction: Some(OrderDirection::Ascending),
                    ..Default::default()
                },
            })
            .await
            .unwrap()
            .result;

        let records: HashMap<String, dns::DnsRecord> = existing_records
            .into_iter()
            .filter(|x| matches!(x.content, dns::DnsContent::A { .. }) && !x.name.eq(&zone_name))
            .map(|x| {
                (
                    String::from(x.name.strip_suffix(&format!(".{}", zone_name)).unwrap()),
                    // match x.content {
                    //     dns::DnsContent::A { content } => content,
                    //     _ => Ipv4Addr::new(0, 0, 0, 0),
                    // },
                    x,
                )
            })
            //.filter(|(name, _)| name.is_some())
            //.map(|(name, ip)| (String::from(name.unwrap()), ip))
            //.filter(|(name, _)| !name.ends_with(zone_name.as_str()))
            .collect();

        Ok(records)
    }

    async fn add_record(&self, name: &String, ip: &Ipv4Addr) -> Result<(), Error> {
        let response = self
            .client
            .request(&dns::CreateDnsRecord {
                zone_identifier: &self.zone_id,
                params: dns::CreateDnsRecordParams {
                    name: &name,
                    content: dns::DnsContent::A { content: *ip },
                    priority: None,
                    proxied: None,
                    ttl: None,
                },
            })
            .await;
        print_response(response);
        Ok(())
    }

    async fn update_record(
        &self,
        record: &dns::DnsRecord,
        name: &String,
        ip: &Ipv4Addr,
    ) -> Result<(), Error> {
        let response = self
            .client
            .request(&dns::UpdateDnsRecord {
                zone_identifier: &self.zone_id,
                identifier: &record.id,
                params: dns::UpdateDnsRecordParams {
                    name: name,
                    content: dns::DnsContent::A { content: *ip },
                    proxied: None,
                    ttl: None,
                },
            })
            .await;
        print_response(response);
        Ok(())
    }

    async fn delete_record(&self, record: &dns::DnsRecord) -> Result<(), Error> {
        let response = self
            .client
            .request(&dns::DeleteDnsRecord {
                zone_identifier: &self.zone_id,
                identifier: &record.id,
            })
            .await;
        print_response(response);
        Ok(())
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    loop {
        let network = get_zt_network().await.unwrap();
        let members = get_zt_ips().await.unwrap();

        let zone_identifier = env::var("CF_ZONE_ID").unwrap();
        let dns = CloudflareDNS::new(zone_identifier);
        let records = dns.get_records().await.unwrap();

        for (name, ip) in &members {
            if !records.contains_key(name) {
                println!("Adding record {:?} with {:?}", name, ip);
                dns.add_record(&name, &ip).await.unwrap();
                continue;
            }

            if let dns::DnsContent::A { content } = records[name].content {
                if *ip != content {
                    println!(
                        "Updating {:?} with {:?} (existing is {:?}))",
                        name, ip, records[name]
                    );
                    dns.update_record(&records[name], &name, &ip).await.unwrap();
                }
            }
        }

        for (name, record) in records.iter() {
            if members.contains_key(name) {
                continue;
            }
            if let dns::DnsContent::A { content } = record.content {
                let mut in_pool = false;
                for pool in network.config.ip_assignment_pools.iter() {
                    in_pool =
                        in_pool || content >= pool.ip_range_start && content <= pool.ip_range_end;
                }
                if !in_pool {
                    continue;
                }
            }
            println!("Deleting record {:?}", record);
            dns.delete_record(record).await.unwrap();
        }

        println!("Going to sleep now");

        sleep(Duration::from_millis(60000)).await;
    }
}
