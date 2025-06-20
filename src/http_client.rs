use anyhow::{Error, Result};
use rand::prelude::SliceRandom;
use rand::Rng;
use reqwest::Client;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use tracing::debug;

#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    #[error("Failed to bind IP {0}: {1}")]
    BindFailed(IpAddr, Error),
}

#[derive(Debug, Default, Clone, Copy)]
pub enum IpSelectAlgorithm {
    #[default]
    RoundRobin,
    Random,
}

#[derive(Debug, Clone, Default)]
pub struct HttpClient {
    clients: Arc<Vec<Client>>,
    algorithm: IpSelectAlgorithm,
    round_robin_index: Arc<Mutex<usize>>,
    last_random_ip: Arc<Mutex<Option<usize>>>,
}

impl HttpClient {
    pub fn new(ips: Vec<IpAddr>, algorithm: IpSelectAlgorithm) -> Result<Self, HttpClientError> {
        let clients = if ips.is_empty() {
            vec![Client::new()]
        } else {
            ips.iter()
                .map(|&ip| {
                    Client::builder()
                        .local_address(Some(ip))
                        .build()
                        .map_err(|e| HttpClientError::BindFailed(ip, e.into()))
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        Ok(Self {
            clients: Arc::new(clients),
            algorithm,
            round_robin_index: Arc::new(Mutex::new(0)),
            last_random_ip: Arc::new(Mutex::new(None)),
        })
    }

    pub fn get_client(&self) -> Client {
        match self.clients.len() {
            0 => unreachable!(),
            1 => self.clients[0].clone(),
            _ => self.select_client(),
        }
    }

    /// 多IP选择算法
    fn select_client(&self) -> Client {
        let index = match self.algorithm {
            IpSelectAlgorithm::RoundRobin => {
                let mut idx = self.round_robin_index.lock().unwrap();
                let selected = *idx;
                *idx = (*idx + 1) % self.clients.len();
                selected
            }
            IpSelectAlgorithm::Random => {
                let mut last_idx = self.last_random_ip.lock().unwrap();
                let candidates: Vec<usize> = (0..self.clients.len())
                    .filter(|&i| Some(i) != *last_idx)
                    .collect();

                let selected = if candidates.is_empty() {
                    rand::thread_rng().gen_range(0..self.clients.len())
                } else {
                    *candidates.choose(&mut rand::thread_rng()).unwrap()
                };

                *last_idx = Some(selected);
                selected
            }
        };
        debug!("selected ip index: {}", index);
        self.clients[index].clone()
    }
}
