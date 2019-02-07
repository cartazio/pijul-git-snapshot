use super::{Change, ChangeContext, NewEdge, Patch, PatchHeader, UnsignedPatch};
use base64;
use serde_json;
use std::str::from_utf8;
use std::collections::HashMap;
use {EdgeFlags, Hash, Key, LineId};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PrettyPatch {
    pub header: PatchHeader,
    pub dependencies: Vec<String>,
    pub changes: Vec<PrettyChange>,
}

impl Patch {
    pub fn to_pretty(&self) -> PrettyPatch {
        match *self {
            Patch::Signed0 | Patch::Unsigned0 => {
                panic!("refusing to interact with old patch version")
            }
            Patch::Unsigned(ref patch) => patch.to_pretty(),
        }
    }
}

impl UnsignedPatch {
    pub fn to_pretty(&self) -> PrettyPatch {
        let deps = self.dependencies.iter().map(|x| x.clone()).zip(1..).collect();
        PrettyPatch {
            header: self.header.clone(),
            dependencies: self.dependencies.iter().map(|x| x.to_base58()).collect(),
            changes: self.changes.iter().map(|x| x.to_pretty(&deps)).collect(),
        }
    }
}

impl PrettyPatch {
    pub fn to_patch(&self) -> UnsignedPatch {
        let deps_vec:Vec<_> = self.dependencies
            .iter()
            .map(|x| Hash::from_base58(x).unwrap())
            .collect();

        UnsignedPatch {
            header: self.header.clone(),
            dependencies: self.dependencies
                .iter()
                .map(|x| Hash::from_base58(x).unwrap())
                .collect(),
            changes: self.changes.iter().map(|x| x.to_change(&deps_vec)).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PrettyChange {
    NewNodes {
        up_context: Vec<String>,
        down_context: Vec<String>,
        flag: u8,
        line_num: u64,
        nodes: Vec<serde_json::Value>,
        inode: String,
    },
    NewEdges {
        previous: u8,
        flag: u8,
        edges: Vec<PrettyNewEdge>,
        inode: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrettyNewEdge {
    pub from: String,
    pub to: String,
    pub introduced_by: usize,
}

impl PrettyNewEdge {
    fn to_new_edge(&self, deps: &[Hash]) -> NewEdge {
        NewEdge {
            from: read_key(deps, &self.from),
            to: read_key(deps, &self.to),
            introduced_by: Some(deps[self.introduced_by].clone()),
        }
    }
}

impl NewEdge {
    fn to_pretty(&self, deps: &HashMap<Hash, usize>) -> PrettyNewEdge {
        PrettyNewEdge {
            from: print_key(deps, &self.from),
            to: print_key(deps, &self.to),
            introduced_by: if let Some(ref i) = self.introduced_by {
                *deps.get(i).unwrap()
            } else {
                0
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Filename {
    name: String,
    flag: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Binary {
    binary: String,
}

fn print_key(deps: &HashMap<Hash, usize>, key: &Key<Option<Hash>>) -> String {
    if let Some(ref k) = key.patch {
        if *k == Hash::None {
            String::new()
        } else {
            format!("{}.{}", deps.get(&k).unwrap(), key.line.to_base58())
        }
    } else {
        format!("{}", key.line.to_base58())
    }
}

fn read_key(deps: &[Hash], key: &str) -> Key<Option<Hash>> {
    let mut it = key.split('.');
    match (it.next(), it.next()) {
        (Some(a), Some(b)) =>
            Key {
                patch: Some(deps[a.parse::<usize>().unwrap()].clone()),
                line: LineId::from_base58(b).unwrap(),
            },
        (Some(a), None) => Key {
            patch: None,
            line: LineId::from_base58(a).unwrap(),
        },
        _ => Key {
            patch: Some(Hash::None),
            line: LineId(0)
        }
    }
}

impl Change<ChangeContext<Hash>> {
    fn to_pretty(&self, deps: &HashMap<Hash, usize>) -> PrettyChange {
        match *self {
            Change::NewNodes {
                ref up_context,
                ref down_context,
                flag,
                line_num,
                ref nodes,
                ref inode,
            } => PrettyChange::NewNodes {
                up_context: up_context.iter().map(|x| print_key(&deps, x)).collect(),
                down_context: down_context.iter().map(|x| print_key(&deps, x)).collect(),
                flag: flag.bits(),
                line_num: line_num.0,
                nodes: if flag.contains(EdgeFlags::FOLDER_EDGE) {
                    nodes
                        .iter()
                        .map(|x| {
                            if x.len() >= 2 {
                                serde_json::to_value(Filename {
                                    name: from_utf8(&x[2..]).unwrap().to_string(),
                                    flag: ((x[0] as u16) << 8) | (x[1] as u16),
                                }).unwrap()
                            } else {
                                serde_json::to_value("").unwrap()
                            }
                        })
                        .collect()
                } else {
                    nodes
                        .iter()
                        .map(|x| {
                            if let Ok(x) = from_utf8(x) {
                                serde_json::to_value(x).unwrap()
                            } else {
                                serde_json::to_value(Binary {
                                    binary: base64::encode(x),
                                }).unwrap()
                            }
                        })
                        .collect()
                },
                inode: print_key(deps, inode),
            },
            Change::NewEdges {
                previous,
                flag,
                ref edges,
                ref inode,
            } => PrettyChange::NewEdges {
                previous: previous.bits(),
                flag: flag.bits(),
                inode: print_key(deps, inode),
                edges: edges.iter().map(|x| x.to_pretty(deps)).collect(),
            },
        }
    }
}

impl PrettyChange {
    pub fn to_change(&self, deps: &[Hash]) -> Change<ChangeContext<Hash>> {
        match *self {
            PrettyChange::NewNodes {
                ref up_context,
                ref down_context,
                flag,
                line_num,
                ref nodes,
                ref inode,
            } => Change::NewNodes {
                up_context: up_context.iter().map(|x| read_key(deps, x)).collect(),
                down_context: down_context.iter().map(|x| read_key(deps, x)).collect(),
                flag: EdgeFlags::from_bits_truncate(flag),
                line_num: LineId(line_num),
                nodes: nodes
                    .iter()
                    .map(|x| {
                        let filename: Result<Filename, _> = serde_json::from_value(x.clone());
                        if let Ok(filename) = filename {
                            let mut v = Vec::new();
                            v.push((filename.flag >> 8) as u8);
                            v.push(filename.flag as u8);
                            v.extend(filename.name.as_bytes());
                            return v;
                        }
                        let binary: Result<Binary, _> = serde_json::from_value(x.clone());
                        if let Ok(binary) = binary {
                            return base64::decode(&binary.binary).unwrap();
                        }
                        let string: String = serde_json::from_value(x.clone()).unwrap();
                        string.into()
                    })
                    .collect(),
                inode: read_key(deps, inode),
            },
            PrettyChange::NewEdges {
                previous,
                flag,
                ref edges,
                ref inode,
            } => Change::NewEdges {
                previous: EdgeFlags::from_bits_truncate(previous),
                flag: EdgeFlags::from_bits_truncate(flag),
                inode: read_key(deps, inode),
                edges: edges.iter().map(|x| x.to_new_edge(deps)).collect(),
            },
        }
    }
}
