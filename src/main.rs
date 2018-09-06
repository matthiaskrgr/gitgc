#![feature(tool_lints)]

#![warn(
    ellipsis_inclusive_range_patterns,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_code,
    unused,
    rust_2018_compatibility,
    rust_2018_idioms
)]

#![warn(
    clippy::all,
    clippy::correctness,
    clippy::perf,
    clippy::complexity,
    clippy::style,
    clippy::pedantic,
    clippy::nursery,
    clippy::shadow_reuse,
    clippy::shadow_same,
    clippy::shadow_unrelated,
    clippy::pub_enum_variant_names,
    clippy::string_add,
    clippy::string_add_assign,
    clippy::needless_borrow
)]


use std::process::Command;
use std::env;
use std::io::{stdout, Write};

use walkdir::WalkDir;
use git2::Repository;
use humansize::{file_size_opts as options, FileSize};

fn size_diff_format(size_before: u64, size_after: u64, dspl_sze_before: bool) -> String {
    let size_diff: i64 = size_after as i64 - size_before as i64;
    let sign = if size_diff > 0 { "+" } else { "" };
    let size_after_human_readable = size_after.file_size(options::DECIMAL).unwrap();
    let humansize_opts = options::FileSizeOpts {
        allow_negative: true,
        ..options::DECIMAL
    };
    let size_diff_human_readable = size_diff.file_size(humansize_opts).unwrap();
    let size_before_human_readabel = size_before.file_size(options::DECIMAL).unwrap();
    // percentage
    let percentage: f64 =
        ((size_after as f64 / size_before as f64) * f64::from(100)) - f64::from(100);
    // format
    let percentage = format!("{:.*}", 2, percentage);

    if size_before == size_after {
        if dspl_sze_before {
            format!(
                "{} => {}",
                size_before_human_readabel, size_after_human_readable
            )
        } else {
            size_after_human_readable.to_string()
        }
    } else if dspl_sze_before {
        format!(
            "{} => {} ({}{}, {}%)",
            size_before_human_readabel,
            size_after_human_readable,
            sign,
            size_diff_human_readable,
            percentage
        )
    } else {
        format!(
            "{} ({}{}, {}%)",
            size_after_human_readable, sign, size_diff_human_readable, percentage
        )
    }
}

fn size_git_repo(repo: &std::path::PathBuf) -> u64 {
    let output = match Command::new("git")
        .arg("count-objects")
        .arg("-v")
        .current_dir(&repo)
        .output()
    {
        Ok(output) => output,
        Err(_e) => panic!("error!! count objects"),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut size = 0;
    for line in stdout.lines() {
        if line.starts_with("size:") || line.starts_with("size-pack:")
            || line.starts_with("size-garbage:")
        {
            //println!("line: {}", line);
            let v: Vec<&str> = line.split(' ').collect();
            let digit = v.last().unwrap();
            let numb = digit.parse::<u64>().unwrap();
            size += numb;
        }
    }
    //println!("out:\n {}", String::from_utf8_lossy(&output.stdout));

    size * 1024 // try to manually convert from kilobyte to byte.
                // there seems to be no easy reliable way to get byte number from git count-objects
}

fn main() {
    let mut global_size_before = 0;
    let mut global_size_after = 0;
    println!("Searching for repos ...");

    let mut list_of_repos = Vec::new();
    let cwd = env::current_dir().unwrap();
    for entry in WalkDir::new(cwd) {
        let entry = entry.unwrap();
        let path = entry.path();
        // git repo ?
        match Repository::open(&path) {
            Err(_e) => continue, // not repo, ignore
            Ok(repo) => {
                let repopath = repo.path();
                if !list_of_repos.contains(&repopath.to_path_buf()) {
                    // collect repos
                    println!("found repo: {:?}", repopath);
                    list_of_repos.push(repopath.to_path_buf());
                }
            } // Ok()
        } // match
    } // for

    println!("Recompressing...");
    for repo in &list_of_repos {
        let size_before = size_git_repo(repo);
        global_size_before += size_before;
        let sb_human_readable = size_before.file_size(options::DECIMAL).unwrap();

        println!("Repo: {:?}: {} => ", &repo, sb_human_readable);
        // flush stdout for incremental print
        match stdout().flush() {
            // ignore errors
            Ok(_ok) => {}
            Err(_e) => {}
        }
        // delete all history of all checkouts and so on.
        // this will enable us to remove *all* dangling commits
        let _ = Command::new("git")
            .arg("reflog")
            .arg("expire")
            .arg("--expire=1.minute")
            .arg("--all")
            .current_dir(&repo)
            .status();

        let _ = Command::new("git")
            .arg("pack-refs")
            .arg("--all")
            .arg("--prune")
            .current_dir(&repo)
            .status();
        // actually recompress repo from scratch
        let _ = Command::new("git")
            .arg("gc")
            .arg("--aggressive")
            .arg("--prune=now")
            .current_dir(&repo)
            .status();
        // recompute size
        let size_after = size_git_repo(repo);
        global_size_after += size_after;

        let text = size_diff_format(size_before, size_after, false);
        println!("Repo: {:?}: {} => {}", &repo, sb_human_readable, text)
    }

    println!("Total:");
    let summary = size_diff_format(global_size_before, global_size_after, true);
    println!("{}", summary);
}
