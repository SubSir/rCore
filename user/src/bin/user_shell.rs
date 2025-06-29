#![no_std]
#![no_main]
#![allow(clippy::println_empty_string)]

extern crate alloc;
use crate::alloc::string::ToString;

#[macro_use]
extern crate user_lib;

const LF: u8 = 0x0au8;
const CR: u8 = 0x0du8;
const DL: u8 = 0x7fu8;
const BS: u8 = 0x08u8;
const LINE_START: &str = " >> ";

use alloc::string::String;
use alloc::vec::Vec;
use user_lib::console::getchar;
use user_lib::{
    OpenFlags, cd, close, dup, exec, fork, ls, mkdir, mv, open, pipe, read, rm, waitpid,
};

#[derive(Debug)]
struct ProcessArguments {
    input: String,
    output: String,
    args_copy: Vec<String>,
    args_addr: Vec<*const u8>,
}

impl ProcessArguments {
    pub fn new(command: &str) -> Self {
        let args: Vec<_> = command.split(' ').collect();
        let mut args_copy: Vec<String> = args
            .iter()
            .filter(|&arg| !arg.is_empty())
            .map(|&arg| {
                let mut string = String::new();
                string.push_str(arg);
                string.push('\0');
                string
            })
            .collect();

        // redirect input
        let mut input = String::new();
        if let Some((idx, _)) = args_copy
            .iter()
            .enumerate()
            .find(|(_, arg)| arg.as_str() == "<\0")
        {
            input = args_copy[idx + 1].clone();
            args_copy.drain(idx..=idx + 1);
        }

        // redirect output
        let mut output = String::new();
        if let Some((idx, _)) = args_copy
            .iter()
            .enumerate()
            .find(|(_, arg)| arg.as_str() == ">\0")
        {
            output = args_copy[idx + 1].clone();
            args_copy.drain(idx..=idx + 1);
        }

        let mut args_addr: Vec<*const u8> = args_copy.iter().map(|arg| arg.as_ptr()).collect();
        args_addr.push(core::ptr::null::<u8>());

        Self {
            input,
            output,
            args_copy,
            args_addr,
        }
    }
}

fn edit_path(current_path: String, command: &str) -> String {
    let mut components = Vec::new();

    if command.starts_with('/') {
        components = Vec::new();
    } else {
        components = current_path
            .split('/')
            .filter(|&s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
    }

    for part in command.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            x => {
                components.push(x.to_string());
            }
        }
    }

    let mut result = String::from("/");
    result.push_str(&components.join("/"));
    while result.contains("//") {
        result = result.replace("//", "/");
    }
    result
}
#[unsafe(no_mangle)]
pub fn main() -> i32 {
    println!("Rust user shell");
    let mut line: String = String::new();
    let mut current_path: String = String::from("/");
    let mut current_inode_id: usize = 0;
    print!("{}", current_path.clone() + LINE_START);
    loop {
        let c = getchar();
        match c {
            LF | CR => {
                println!("");
                if !line.is_empty() {
                    let splited: Vec<_> = line.as_str().split('|').collect();
                    let process_arguments_list: Vec<_> = splited
                        .iter()
                        .map(|&cmd| ProcessArguments::new(cmd))
                        .collect();
                    let mut valid = true;
                    for (i, process_args) in process_arguments_list.iter().enumerate() {
                        if i == 0 {
                            if !process_args.output.is_empty() {
                                valid = false;
                            }
                        } else if i == process_arguments_list.len() - 1 {
                            if !process_args.input.is_empty() {
                                valid = false;
                            }
                        } else if !process_args.output.is_empty() || !process_args.input.is_empty()
                        {
                            valid = false;
                        }
                    }
                    if process_arguments_list.len() == 1 {
                        valid = true;
                    }
                    if !valid {
                        println!("Invalid command: Inputs/Outputs cannot be correctly binded!");
                    } else {
                        // create pipes
                        let mut pipes_fd: Vec<[usize; 2]> = Vec::new();
                        if !process_arguments_list.is_empty() {
                            for _ in 0..process_arguments_list.len() - 1 {
                                let mut pipe_fd = [0usize; 2];
                                pipe(&mut pipe_fd);
                                pipes_fd.push(pipe_fd);
                            }
                        }
                        let mut children: Vec<_> = Vec::new();
                        for (i, process_argument) in process_arguments_list.iter().enumerate() {
                            let args_copy = &process_argument.args_copy;
                            if args_copy[0] == "mkdir\0" {
                                if args_copy.len() != 2 {
                                    println!("Invalid command: mkdir requires one argument");
                                    continue;
                                }
                                if mkdir(current_inode_id, args_copy[1].as_str()) == -1 {
                                    println!("Error when creating directory {}", args_copy[1]);
                                };
                                continue;
                            }
                            if args_copy[0] == "rm\0" {
                                if args_copy.len() != 2 {
                                    println!("Invalid command: rm requires one argument");
                                    continue;
                                }
                                if rm(current_inode_id, args_copy[1].as_str()) == -1 {
                                    println!("Error when removing file {}", args_copy[2]);
                                };
                                continue;
                            }
                            if args_copy[0] == "mv\0" {
                                if args_copy.len() != 3 {
                                    println!("Invalid command: mv requires two arguments");
                                    continue;
                                }
                                if mv(
                                    current_inode_id,
                                    args_copy[1].as_str(),
                                    args_copy[2].as_str(),
                                ) == -1
                                {
                                    println!(
                                        "Error when moving file {} to {}",
                                        args_copy[1], args_copy[2]
                                    );
                                }
                                continue;
                            }
                            if args_copy[0] == "cd\0" {
                                if args_copy.len() != 2 {
                                    println!("Invalid command: cd requires one argument");
                                    continue;
                                }
                                let inode_id = cd(current_inode_id, args_copy[1].as_str());
                                if inode_id != -1 {
                                    current_inode_id = inode_id as usize;
                                    current_path =
                                        edit_path(current_path.clone(), args_copy[1].as_str());
                                } else {
                                    println!("Error when changing directory to {}", args_copy[1]);
                                }
                                continue;
                            }
                            if args_copy[0] == "ls\0" {
                                if ls(current_inode_id) == -1 {
                                    println!("Error when listing directory");
                                };
                                continue;
                            }
                            if args_copy[0] == "cat\0" {
                                if args_copy.len() != 2 {
                                    println!("Invalid command: cat requires one argument");
                                    continue;
                                }
                                let fd = open(
                                    current_inode_id,
                                    args_copy[1].as_str(),
                                    OpenFlags::RDONLY,
                                );
                                if fd == -1 {
                                    panic!("Error occured when opening file");
                                }
                                let fd = fd as usize;
                                let mut buf = [0u8; 256];
                                loop {
                                    let size = read(fd, &mut buf) as usize;
                                    if size == 0 {
                                        break;
                                    }
                                    print!("{}", core::str::from_utf8(&buf[..size]).unwrap());
                                }
                                close(fd);
                                continue;
                            }
                            let pid = fork();
                            if pid == 0 {
                                let input = &process_argument.input;
                                let output = &process_argument.output;
                                let args_copy = &process_argument.args_copy;
                                let args_addr = &process_argument.args_addr;
                                // redirect input
                                if !input.is_empty() {
                                    let input_fd =
                                        open(current_inode_id, input.as_str(), OpenFlags::RDONLY);
                                    if input_fd == -1 {
                                        println!("Error when opening file {}", input);
                                        return -4;
                                    }
                                    let input_fd = input_fd as usize;
                                    close(0);
                                    assert_eq!(dup(input_fd), 0);
                                    close(input_fd);
                                }
                                // redirect output
                                if !output.is_empty() {
                                    let output_fd = open(
                                        current_inode_id,
                                        output.as_str(),
                                        OpenFlags::CREATE | OpenFlags::WRONLY,
                                    );
                                    if output_fd == -1 {
                                        println!("Error when opening file {}", output);
                                        return -4;
                                    }
                                    let output_fd = output_fd as usize;
                                    close(1);
                                    assert_eq!(dup(output_fd), 1);
                                    close(output_fd);
                                }
                                // receive input from the previous process
                                if i > 0 {
                                    close(0);
                                    let read_end = pipes_fd.get(i - 1).unwrap()[0];
                                    assert_eq!(dup(read_end), 0);
                                }
                                // send output to the next process
                                if i < process_arguments_list.len() - 1 {
                                    close(1);
                                    let write_end = pipes_fd.get(i).unwrap()[1];
                                    assert_eq!(dup(write_end), 1);
                                }
                                // close all pipe ends inherited from the parent process
                                for pipe_fd in pipes_fd.iter() {
                                    close(pipe_fd[0]);
                                    close(pipe_fd[1]);
                                }
                                // execute new application
                                if exec(
                                    current_inode_id,
                                    args_copy[0].as_str(),
                                    args_addr.as_slice(),
                                ) == -1
                                {
                                    println!("Error when executing!");
                                    return -4;
                                }
                                unreachable!();
                            } else {
                                children.push(pid);
                            }
                        }
                        for pipe_fd in pipes_fd.iter() {
                            close(pipe_fd[0]);
                            close(pipe_fd[1]);
                        }
                        let mut exit_code: i32 = 0;
                        for pid in children.into_iter() {
                            let exit_pid = waitpid(pid as usize, &mut exit_code);
                            assert_eq!(pid, exit_pid);
                            //println!("Shell: Process {} exited with code {}", pid, exit_code);
                        }
                    }
                    line.clear();
                }
                print!("{}", current_path.clone() + LINE_START);
            }
            BS | DL => {
                if !line.is_empty() {
                    print!("{}", BS as char);
                    print!(" ");
                    print!("{}", BS as char);
                    line.pop();
                }
            }
            _ => {
                print!("{}", c as char);
                line.push(c as char);
            }
        }
    }
}
