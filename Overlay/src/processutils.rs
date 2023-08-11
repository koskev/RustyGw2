use std::collections::HashMap;

use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd;

pub fn find_wine_process(name: &str) -> Option<i32> {
    println!("### Readin proc in rust");
    let dirs = std::fs::read_dir("/proc").unwrap();
    for d in dirs {
        let d = d.unwrap();
        if d.path().is_dir()
            && d.file_name()
                .to_str()
                .unwrap()
                .to_string()
                .chars()
                .all(|c| c.is_numeric() && c.is_alphanumeric())
        {
            // It is a process dir
            let cmdline_path = d.path().join("cmdline");
            if cmdline_path.exists() {
                let cmdline = std::fs::read_to_string(cmdline_path);
                match cmdline {
                    Ok(line) => {
                        if line.find(name).is_some() {
                            let pid = d.file_name().to_str().unwrap().parse::<i32>().unwrap_or(-1);
                            println!("Found {} with pid {}", name, pid);
                            return Some(pid);
                        }
                    }
                    Err(_) => (),
                }
            }
        }
    }
    None
}

pub fn is_child_running(pid: i32) -> bool {
    let ret = waitpid(unistd::Pid::from_raw(pid), Some(WaitPidFlag::WNOHANG));
    return ret.is_ok() && ret.unwrap() == WaitStatus::StillAlive;
}

pub fn get_env_val(list_of_envs: Vec<String>, env_to_find: &str) -> String {
    for env in list_of_envs {
        let tokens = env.split_once("=");
        if tokens.is_some() && tokens.unwrap().0 == env_to_find {
            return tokens.unwrap().1.to_string();
        }
    }
    return "".to_string();
}

fn start_process(cmd: &str, args: Vec<&str>, env: HashMap<String, String>) -> u32 {
    let process = std::process::Command::new(cmd)
        .args(args)
        .envs(env)
        .spawn()
        .unwrap();
    process.id()
}

pub fn get_environment(pid: i32) -> HashMap<String, String> {
    let proc_path = format!("/proc/{}/environ", pid);
    let env_path = std::path::Path::new(&proc_path);
    let mut env_data: HashMap<String, String> = HashMap::new();
    if env_path.exists() {
        let data = std::fs::read_to_string(env_path);
        match data {
            Ok(d) => {
                let tokens = d.split("\0");
                tokens.into_iter().for_each(|t| {
                    let key_val = t.split_once("=");
                    if key_val.is_some() {
                        env_data.insert(
                            key_val.unwrap().0.to_string(),
                            key_val.unwrap().1.to_string(),
                        );
                    }
                });
            }
            Err(_) => (),
        }
    }
    env_data
}

pub fn remove_from_env(
    env: HashMap<String, String>,
    to_remove: Vec<&str>,
) -> HashMap<String, String> {
    let env = env
        .into_iter()
        .filter(|(key, _val)| {
            let str_key: &str = key;
            !to_remove.contains(&str_key)
        })
        .collect();

    env
}

pub fn start_gw2_helper(pid: i32, helper_path: &str) -> i32 {
    let env = get_environment(pid);

    let envs_to_remove = vec![
        "WINESERVERSOCKET",
        "WINELOADERNOEXEC",
        "WINEPRELOADRESERVE",
        "LD_PRELOAD",
    ];
    let env = remove_from_env(env, envs_to_remove);

    let wine = env.get("WINE").unwrap_or(&"wine".to_string()).clone();
    let prefix = env
        .get("WINEPREFIX")
        .unwrap_or(&"~/.wine".to_string())
        .clone();
    println!("Wine: {} prefix: {}", wine, prefix);

    let mut pid: i32 = -1;
    if !helper_path.is_empty() {
        pid = start_process(&wine, vec![&helper_path], env) as i32;
    }
    pid
}
