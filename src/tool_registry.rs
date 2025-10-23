use serde_json::Value;

#[derive(Clone)]
pub struct ToolRegistry {
    schemas: Value,
}

impl ToolRegistry {
    pub fn new() -> Self {
        // Single source of truth for "tools" schema the LLM sees
        let schemas = serde_json::json!([
            {
                "type": "function",
                "function": {
                    "name": "list_dir",
                    "description":
                        "Lists all files and directories in the given path",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Directory path to list"
                            }
                        },
                        "required": ["path"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description":
                        "Returns the content of the file for the given path",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file"
                            },
                            "start_line": {
                                "type": "number",
                                "description":
                                    "Starting line (optional, default 1)"
                            },
                            "end_line": {
                                "type": "number",
                                "description":
                                    "Ending line (optional, default start+200)"
                            }
                        },
                        "required": ["path"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "run_shell",
                    "description":
                        "Executes a shell command with a 30-second timeout. \
                         Dangerous commands like rm, sudo, dd are blocked.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "command": {
                                "type": "string",
                                "description": "The shell command to execute"
                            }
                        },
                        "required": ["command"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "write_file",
                    "description":
                        "Writes content to a file. Creates a file if absent.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path of file to write"
                            },
                            "content": {
                                "type": "string",
                                "description":
                                    "Content to write into the file"
                            }
                        },
                        "required": ["path", "content"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "search_in_files",
                    "description":
                        "Recursive search for a regex pattern with \
                         gitignore-style filtering.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "pattern": {
                                "type": "string",
                                "description":
                                    "Regular expression (Rust syntax)"
                            },
                            "path": {
                                "type": "string",
                                "description":
                                    "File or directory to search"
                            },
                            "case_sensitive": {
                                "type": "boolean",
                                "description":
                                    "Case-sensitive match (default true)"
                            }
                        },
                        "required": ["pattern", "path"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "edit_file",
                    "description":
                        "Edits a file by replacing an existing string.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file"
                            },
                            "old_str": {
                                "type": "string",
                                "description": "String to be replaced"
                            },
                            "new_str": {
                                "type": "string",
                                "description": "Replacement string"
                            }
                        },
                        "required": ["path", "old_str", "new_str"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "insert_in_file",
                    "description": "Insert content before or after a specific anchor (unique string) in a file.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "The file path to modify (e.g., 'src/main.py')"
                            },
                            "anchor": {
                                "type": "string",
                                "description": "A unique string that exists in the file to use as insertion point. Should be specific enough not to have duplicates."
                            },
                            "content": {
                                "type": "string",
                                "description": "The content to insert into the file."
                            },
                            "position": {
                                "type": "string",
                                "enum": ["before", "after"],
                                "description": "Whether to insert content before or after the anchor."
                            },
                            "newline": {
                                "type": "boolean",
                                "description": "Add newlines around the inserted content. Default: true",
                                "default": true
                            }
                        },
                        "required": ["path", "anchor", "content", "position"]
                    }
                }
            },
            {
                "type": "function",
                "function": {
                    "name": "ask_orackle",
                    "description": "Ask Orackle for insights when stuck with complex problems. Orackle is a read-only expert agent that provides strategic guidance and alternative approaches.",
                    "parameters": {
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "Detailed description of the problem or situation where the main agent is stuck"
                            }
                        },
                        "required": ["query"]
                    }
                }
            }
        ]);
        Self { schemas }
    }

    pub fn schemas(&self) -> &Value {
        &self.schemas
    }
}