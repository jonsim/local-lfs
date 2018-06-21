use std::env;
use std::fmt;
use std::process;

struct Args {
    port: Option<i32>,
    store_path: Option<String>,
}

impl fmt::Debug for Args {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Args {{ port: {:?}, store_path: {:?} }}",
                self.port, self.store_path)
    }
}

impl Args {
    fn _usage() {
        let prog = env::current_exe();
        let prog = match prog.as_ref() {
            Ok(p) => p.file_name().expect("Executable path does not have basename")
                      .to_str().expect("Executable path contains invalid UTF8"),
            Err(_) => "local-lfs",
        };
        println!("\
usage: {} [-h] [-p PORT] [-s PATH]

A simple git-lfs server which can echo git commits to an external server and
store large file objects in a separate local store.

optional arguments:
    -p PORT, --port PORT    The port the server will be hosted on. Defaults to
            9090.
    -s PATH, --store PATH   Path to a directory in which the large file object
            store will be created. This may be a folder backed by cloud storage
            client (e.g. Dropbox, Google Drive etc). Defaults to './lfo-store'.
    -h, --help              Print this message and exit.",
        prog);
        process::exit(0);
    }

    fn parse_cmdline() -> Args {
        let mut iter = env::args();
        iter.next();    // Skip arg0

        let mut port: Option<i32> = None;
        let mut store_path: Option<String> = None;
        while let Some(arg) = iter.next() {
            match arg.as_ref() {
                "-h" | "--help" => Args::_usage(),
                "-p" | "--port" => port = Some(iter.next()
                                .expect("Port number must be given")
                                .parse()
                                .expect("Port number must be numeric")),
                "-s" | "--store" => store_path = Some(iter.next()
                                .expect("Store path must be given")),
                _ => panic!("Unexpected argument"),
            }
        }

        Args { port, store_path }
    }
}

fn main() {
    let args = Args::parse_cmdline();

    println!("{:?}", args);
}
