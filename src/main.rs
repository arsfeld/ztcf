extern crate dotenv;

use async_trait::async_trait;
use dotenv::dotenv;
use reqwest::Error;
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use cloudflare::endpoints::{account, dns, workers, zone};
use cloudflare::framework::{
    async_api::ApiClient,
    async_api::Client,
    auth::Credentials,
    mock::{MockApiClient, NoopEndpoint},
    response::{ApiFailure, ApiResponse, ApiResult},
    Environment, HttpApiClient, HttpApiClientConfig, OrderDirection,
};

#[derive(Deserialize, Debug)]
struct NetworkMemberConfig {
    ipAssignments: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct NetworkMember {
    id: String,
    clock: usize,
    networkId: String,
    nodeId: String,
    controllerId: String,
    config: NetworkMemberConfig,
    hidden: bool,
    name: String,
    description: String,
    lastOnline: usize,
    physicalAddress: String,
    clientVersion: String,
    protocolVersion: u32,
    supportsRulesEngine: bool,
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
        .map(|x| (x.name, x.config.ipAssignments[0].parse::<Ipv4Addr>().unwrap()))
        .collect();
    println!("{:?}", ips);

    return Ok(ips);
}

#[async_trait]
trait DNS {
    async fn get_records(&self) -> Result<HashMap<String, String>, Error>;
}

struct CloudflareDNS {
    client: Client,
    zone_id: String,
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

    async fn get_records(&self) -> Result<HashMap<String, Ipv4Addr>, Error> {
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

        let records: HashMap<String, Ipv4Addr> = existing_records
            .into_iter()
            .map(|x| {
                (
                    match x.name.strip_suffix(&format!(".{}", zone_name)) {
                        Some(name) => name.to_string(),
                        None => x.name.to_string(),
                    },
                    match x.content {
                        dns::DnsContent::A { content } => content,
                        _ => Ipv4Addr::new(0, 0, 0, 0),
                    },
                )
            })
            .collect();

        return Ok(records);
    }

    async fn add_record(&self, name: String, ip: Ipv4Addr) -> Result<(), Error> {
        let response = self.client
            .request(&dns::CreateDnsRecord {
                zone_identifier: &self.zone_id,
                params: dns::CreateDnsRecordParams {
                    name: &name,
                    content: dns::DnsContent::A {
                        content: ip,
                    },
                    priority: None,
                    proxied: None,
                    ttl: None,
                },
            })
            .await;
        println!("{:?}", response);
        return Ok(());
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv().ok();

    let members = get_zt_ips().await?;

    println!("Members: {:?}", members);

    let zone_identifier = env::var("CF_ZONE_ID").unwrap();

    let dns = CloudflareDNS::new(zone_identifier);

    let records = dns.get_records().await?;

    println!("Records: {:?}", records);

    for (name, ip) in members {
        let response = dns.add_record(name, ip).await;
    }

    Ok(())
}
