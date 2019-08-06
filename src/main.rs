use colored::*;
use std::io::*;
use std::process::*;

#[derive(Debug)]
struct Branch {
    name: String,
    commit_hash: String,
}

fn parse_branch(line: String) -> Option<Branch> {
    let parts: Vec<&str> = line.split(" ").collect();
    match parts.as_slice() {
        [name, commit_hash] => Some(Branch {
            name: name.to_string(),
            commit_hash: commit_hash.to_string(),
        }),
        _ => None,
    }
}

struct RemoteBranch {
    state: String,
    name: String,
    commit_hash: String,
}

impl RemoteBranch {
    pub fn parse_line(line: String) -> Option<RemoteBranch> {
        let parts: Vec<&str> = line.split(" ").collect();
        match parts.as_slice() {
            [state, _number, branch_name, commit_hash] => Some(RemoteBranch {
                state: state.to_string(),
                name: branch_name.to_string(),
                commit_hash: commit_hash.to_string(),
            }),
            _ => None,
        }
    }

    pub fn branch(&self) -> Branch {
        Branch {
            name: self.name.to_string(),
            commit_hash: self.commit_hash.to_string(),
        }
    }
}

fn get_remote_branches() -> std::io::Result<impl Iterator<Item = Branch>> {
    // hub pr list -s all -f "%S %i %H %sH%n"
    let git_branch = Command::new("hub")
        .args(vec![
            "pr",
            "list",
            "-s",
            "all",
            "-f",
            "%S %i %H %sH%n",
            "--limit",
            "20",
        ])
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    let git_branches = BufReader::new(git_branch.stdout.unwrap()).lines();
    let branches = git_branches
        .filter_map(|line| RemoteBranch::parse_line(line.unwrap()))
        .filter(|remote_branch| remote_branch.state != "open")
        .map(|remote_branch| remote_branch.branch());
    Ok(branches)
}

fn get_local_branches() -> std::io::Result<impl Iterator<Item = Branch>> {
    let git_branch = Command::new("git")
        .arg("branch")
        .arg("--format")
        .arg("%(refname:short) %(objectname)")
        .stdout(std::process::Stdio::piped())
        .spawn()?;
    let git_branches = BufReader::new(git_branch.stdout.unwrap()).lines();
    let branches = git_branches.filter_map(|line| parse_branch(line.ok()?));
    Ok(branches)
}

fn group_by<T: std::fmt::Debug, F: Fn(&T) -> String>(
    iterator: &mut Iterator<Item = T>,
    f: F,
) -> std::collections::HashMap<String, T> {
    use std::collections::*;
    let mut cache: HashMap<String, T> = Default::default();
    for item in iterator {
        cache.insert(f(&item), item);
    }
    cache
}

fn main() -> std::io::Result<()> {
    let (tx_local, rx_local) = std::sync::mpsc::channel();
    let (tx_remote, rx_remote) = std::sync::mpsc::channel();

    std::thread::spawn(move || {
        log("> Collecting local branches from git...");
        let local_branches = get_local_branches().expect("Can't get local branches");
        let branches_vec: Vec<Branch> = local_branches.collect();
        tx_local
            .send(branches_vec)
            .expect("Can't send local branches");
        log("> Done collecting local branches from git!".green());
    });

    std::thread::spawn(move || {
        log("> Collecting remote branches from GitHub...");
        let remote_branches = get_remote_branches().expect("Can't get remote branches");
        let branches_vec: Vec<Branch> = remote_branches.collect();
        tx_remote
            .send(branches_vec)
            .expect("Can't send local branches");
        log("> Done collecting remote branches from GitHub!".green());
    });

    let local_branches = rx_local.recv().expect("Can't get local branches");
    let remote_branches = rx_remote.recv().expect("Can't get local branches");

    let cache = group_by(&mut remote_branches.iter(), |x| x.commit_hash.to_string());

    for branch in local_branches {
        match cache.get(&branch.commit_hash) {
            None => log(format!(
                "Can't find {} ({})",
                branch.name, branch.commit_hash
            )),
            Some(_) => println!("{}", branch.name),
        };
    }
    Ok(())
}

fn log<'a, T: std::fmt::Display>(text: T) {
    let verbose = std::env::args().any(|x| x == "--verbose");
    if verbose {
        eprintln!("{}", format!("{}", text).dimmed());
    }
}
