extern crate env_logger;
extern crate futures;
#[macro_use]
extern crate log;
extern crate tokio;
extern crate tokio_timer;
extern crate trust_dns_resolver as dns;

use futures::prelude::*;
use tokio_timer::{clock, Delay, Timeout};

const TIMEOUT: ::std::time::Duration = ::std::time::Duration::from_millis(100);
const ERROR_TTL: ::std::time::Duration = ::std::time::Duration::from_secs(10);

fn main() {
    env_logger::init();

    let mut args = ::std::env::args();
    if args.len() < 2 {
        eprintln!("usage: trust-refine <name>");
        ::std::process::exit(64);
    }
    let _ = args.next().unwrap();

    let (config, mut opts) =
        dns::system_conf::read_system_conf().expect("invalid DNS configuration");
    opts.cache_size = 0;

    let mut rt = tokio::runtime::current_thread::Runtime::new().unwrap();

    let (resolver, daemon) = dns::AsyncResolver::new(config, opts);
    while let Some(name) = args.next() {
        rt.spawn(Refine {
            name,
            resolver: resolver.clone(),
            state: State::Init,
        });
    }

    rt.block_on(daemon).unwrap();
}

struct Refine {
    name: String,
    resolver: dns::AsyncResolver,
    state: State,
}

enum State {
    Init,
    Pending(Timeout<dns::BackgroundLookupIp>),
    ValidUntil(Delay),
}

impl Future for Refine {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), ()> {
        loop {
            self.state = match self.state {
                State::Init => {
                    let f = self.resolver.lookup_ip(AsRef::<str>::as_ref(&self.name));
                    State::Pending(Timeout::new(f, TIMEOUT))
                }

                State::Pending(ref mut fut) => match fut.poll() {
                    Ok(Async::NotReady) => return Ok(Async::NotReady),
                    Ok(Async::Ready(lookup)) => {
                        println!("{}: {}", self.name, lookup.query().name());
                        info!(
                            "{} valid for {}s",
                            self.name,
                            (lookup.valid_until() - clock::now()).as_secs(),
                        );
                        State::ValidUntil(Delay::new(lookup.valid_until()))
                    }
                    Err(e) => {
                        error!("{}: {:?}", self.name, e);

                        let valid_until = e
                            .into_inner()
                            .and_then(|e| match e.kind() {
                                dns::error::ResolveErrorKind::NoRecordsFound {
                                    valid_until,
                                    ..
                                } => *valid_until,
                                _ => None,
                            })
                            .unwrap_or_else(|| clock::now() + ERROR_TTL);

                        State::ValidUntil(Delay::new(valid_until))
                    }
                },

                State::ValidUntil(ref mut f) => match f.poll().expect("timer must not fail") {
                    Async::NotReady => return Ok(Async::NotReady),
                    Async::Ready(()) => {
                        debug!("refresh");
                        State::Init
                    }
                },
            };
        }
    }
}
