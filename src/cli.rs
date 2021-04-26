use structopt::StructOpt;
use tunneler_core::server::Strategy;

#[derive(StructOpt)]
pub enum Arguments {
    Client {
        /// The Port on which the Server should listen to for
        /// Requests for this Client
        #[structopt(short = "p")]
        external_port: u16,
        /// The Port on which the Client connects to the Server
        #[structopt(short = "l")]
        listen_port: u32,
        /// The IP of the external Server
        #[structopt(long = "ip")]
        server_ip: String,
        /// The Port of the Target-Endpoint
        #[structopt(long = "target-port")]
        target_port: u32,
        /// The IP of the Target-Endpoint
        #[structopt(long = "target-ip", default_value = "localhost")]
        target_ip: String,
        /// The Path to the Key used for authentication with the Server
        #[structopt(long = "key", short = "k")]
        key_path: Option<String>,
        /// The Number of threads to use for the Runtime
        #[structopt(long = "threads", short = "t")]
        threads: Option<usize>,
    },
    Server {
        /// The Single External port that the Server-Exposes
        #[structopt(short = "p", parse(try_from_str = parse_strategy))]
        external_port: Strategy,
        /// The Port on which the Server should listen for Clients
        #[structopt(short = "l")]
        listen_port: u32,
        /// The Path to the Key used for authenticating Clients
        #[structopt(long = "key", short = "k")]
        key_path: Option<String>,
        /// The Number of threads to use for the Runtime
        #[structopt(long = "threads", short = "t")]
        threads: Option<usize>,
    },
    KeyGen {
        /// The Path where the generated Key should be saved
        #[structopt(long = "key", short = "k")]
        key_path: Option<String>,
    },
}

fn parse_strategy(src: &str) -> Result<Strategy, String> {
    if let Some(_) = src.find(",") {
        let mut result: Vec<u16> = Vec::with_capacity(2);

        for raw_tmp in src.split(",") {
            if let Ok(tmp) = raw_tmp.parse() {
                result.push(tmp);
            }
        }

        return Ok(Strategy::Multiple(result));
    }
    if let Some(_) = src.find("..") {
        if src.len() == 2 {
            return Ok(Strategy::Dynamic(None));
        }

        let parts: Vec<&str> = src.split("..").collect();
        let raw_start = parts.get(0).unwrap();
        let raw_end = parts.get(1).unwrap();

        let start: u16 = raw_start.parse().unwrap();
        let end: u16 = raw_end.parse().unwrap();

        return Ok(Strategy::Dynamic(Some(start..end)));
    }

    let single: u16 = src.parse().unwrap();

    Ok(Strategy::Single(single))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_strategy() {
        assert_eq!(Ok(Strategy::Single(8080)), parse_strategy("8080"))
    }
    #[test]
    fn dynamic_strategy() {
        assert_eq!(Ok(Strategy::Dynamic(None)), parse_strategy(".."))
    }
    #[test]
    fn dynamic_range_strategy() {
        assert_eq!(
            Ok(Strategy::Dynamic(Some(8080..8090))),
            parse_strategy("8080..8090")
        )
    }
    #[test]
    fn multiple_strategy() {
        assert_eq!(
            Ok(Strategy::Multiple(vec![8080, 8081, 8082])),
            parse_strategy("8080,8081,8082")
        )
    }
}
