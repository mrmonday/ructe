//! Rust Compiled Templates is a HTML template system for Rust.
//!
//! Templates in a syntax inspired by
//! [Twirl](https://github.com/playframework/twirl), the Scala-based
//! template engine in
//! [Play framework](https://www.playframework.com/) are translated to
//! Rust language source code, to be compiled together with your
//! program.
//! The template syntax is currently documented only in the readme of
//! [the github repository for ructe](https://github.com/kaj/ructe).
//!
//!
//! # How to use ructe
//!
//! Ructe compiles your templates to rust code that should be compiled with
//! your other rust code, so it needs to be called before compiling.
//! Assuming you use [cargo](http://doc.crates.io/), it can be done like
//! this:
//!
//! First, specify a build script and ructe as a build dependency in
//! `Cargo.toml`:
//!
//! ```toml
//! build = "src/build.rs"
//!
//! [build-dependencies]
//! ructe = "^0.2"
//! ```
//!
//! Then, in the build script, compile all templates found in the templates
//! directory and put the output where cargo tells it to:
//!
//! ```no-run
//! extern crate ructe;
//!
//! use ructe::compile_templates;
//! use std::env;
//! use std::path::PathBuf;
//!
//! fn main() {
//!     let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
//!     let in_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
//!         .join("templates");
//!     compile_templates(&in_dir, &out_dir).expect("compile templates");
//! }
//! ```
//!
//! And finally, include and use the generated code in your code.
//! The file `templates.rs` will contain `mod templates { ... }`,
//! so I just include it in my `main.rs`:
//!
//! ```no-compile
//! include!(concat!(env!("OUT_DIR"), "/templates.rs"));
//! ```
//!
//! When calling a template, the arguments declared in the template will be
//! prepended by a `Write` argument to write the output to.
//! It can be a `Vec<u8>` as a buffer or for testing, or an actual output
//! destination.
//! The return value of a template is `std::io::Result<()>`, which should be
//! `Ok(())` unless writing to the destination fails.
//!
//! ```
//! #[test]
//! fn test_hello() {
//!     let mut buf = Vec::new();
//!     templates::hello(&mut buf, "World").unwrap();
//!     assert_eq!(buf, b"<h1>Hello World!</h1>\n");
//! }
//! ```

#[macro_use]
extern crate nom;

mod spacelike;
mod expression;
mod templateexpression;
mod template;

use nom::IResult::*;
use std::fs::{File, create_dir_all, read_dir};
use std::io::{self, Read, Write};
use std::path::Path;
use template::template;

/// Create a `templates` module in `outdir` containing rust code for
/// all templates found in `indir`.
pub fn compile_templates(indir: &Path, outdir: &Path) -> io::Result<()> {
    File::create(outdir.join("templates.rs")).and_then(|mut f| {
        try!(write!(f,
                    "mod templates {{\n\
                     use std::io::{{self, Write}};\n\
                     use std::fmt::Display;\n\n"));

        let outdir = outdir.join("templates");
        try!(create_dir_all(&outdir));
        try!(handle_entries(&mut f, indir, &outdir));
        write!(f,
               "{}\n}}\n",
               include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
                                    "/src/template_utils.rs")))
    })
}

fn handle_entries(f: &mut Write,
                  indir: &Path,
                  outdir: &Path)
                  -> io::Result<()> {
    let suffix = ".rs.html";
    for entry in try!(read_dir(indir)) {
        let entry = try!(entry);
        let path = entry.path();
        if try!(entry.file_type()).is_dir() {
            if let Some(filename) = entry.file_name().to_str() {
                let outdir = outdir.join(filename);
                try!(create_dir_all(&outdir));
                try!(File::create(outdir.join("mod.rs"))
                    .and_then(|mut f| handle_entries(&mut f, &path, &outdir)));
                try!(write!(f, "pub mod {name};\n\n", name = filename));
            }

        } else if let Some(filename) = entry.file_name().to_str() {
            if filename.ends_with(suffix) {
                println!("cargo:rerun-if-changed={}", path.to_string_lossy());
                let name = &filename[..filename.len() - suffix.len()];
                if try!(handle_template(name, &path, &outdir)) {
                    try!(write!(f,
                                "mod template_{name};\npub use \
                                 self::template_{name}\
                                 ::{name};\n\n",
                                name = name));
                }
            }
        }
    }
    Ok(())
}

fn handle_template(name: &str, path: &Path, outdir: &Path) -> io::Result<bool> {
    let mut input = try!(File::open(path));
    let mut buf = Vec::new();
    try!(input.read_to_end(&mut buf));
    match template(&buf) {
        Done(_, t) => {
            let fname = outdir.join(format!("template_{}.rs", name));
            try!(File::create(fname)
                .and_then(|mut f| t.write_rust(&mut f, name)));
            Ok(true)
        }
        Error(err) => {
            println!("cargo:warning=\
                      Template parse error in {:?}: {}",
                     path,
                     err);
            Ok(false)
        }
        Incomplete(needed) => {
            println!("cargo:warning=\
                      Failed to parse template {:?}: \
                      {:?} needed",
                     path,
                     needed);
            Ok(false)
        }
    }
}

#[cfg(test)]
mod template_utils_test {
    use std::fmt::Display;
    use std::io::{self, Write};
    include!("template_utils.rs");

    #[test]
    fn encoded() {
        let mut buf = Vec::new();
        "a < b".to_html(&mut buf).unwrap();
        assert_eq!(b"a &lt; b", &buf[..]);
    }
    #[test]
    fn raw_html() {
        let mut buf = Vec::new();
        Html("a<b>c</b>").to_html(&mut buf).unwrap();
        assert_eq!(b"a<b>c</b>", &buf[..]);
    }
}
