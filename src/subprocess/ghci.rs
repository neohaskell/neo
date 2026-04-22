use crate::errors::NeoError;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::{Child, ChildStdin, Command};

pub struct GhciSession {
    child: Child,
    stdin: ChildStdin,
}

impl GhciSession {
    pub async fn start() -> miette::Result<Self> {
        let mut child = Command::new("nix")
            .args(["develop", "--command", "bash", "-c", "cabal repl"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| NeoError::SubprocessError { 
                command: "cabal repl".to_string(),
                output: format!("Failed to start GHCi via nix develop: {}", e)
            })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            NeoError::SubprocessError { 
                command: "cabal repl".to_string(),
                output: "Failed to open stdin for GHCi".to_string()
            }
        })?;

        let mut session = Self { child, stdin };
        
        // Wait for initial prompt
        session.wait_for_prompt().await?;
        
        Ok(session)
    }

    pub async fn reload(&mut self) -> miette::Result<Vec<String>> {
        self.stdin.write_all(b":reload\n").await
            .map_err(|e| NeoError::SubprocessError { 
                command: ":reload".to_string(),
                output: format!("Failed to write to GHCi: {}", e)
            })?;
        
        self.wait_for_prompt().await
    }

    async fn wait_for_prompt(&mut self) -> miette::Result<Vec<String>> {
        let stdout = self.child.stdout.as_mut().ok_or_else(|| {
            NeoError::SubprocessError { 
                command: "cabal repl".to_string(),
                output: "Failed to open stdout for GHCi".to_string()
            }
        })?;
        Self::read_until_prompt(stdout).await
    }

    async fn read_until_prompt<R: tokio::io::AsyncRead + Unpin>(reader: &mut R) -> miette::Result<Vec<String>> {
        let mut buffer = Vec::new();
        let mut output_lines = Vec::new();
        let mut byte = [0u8; 1];

        loop {
            match tokio::io::AsyncReadExt::read_exact(reader, &mut byte).await {
                Ok(_) => {
                    buffer.push(byte[0]);
                    let current_str = String::from_utf8_lossy(&buffer);
                    if current_str.ends_with("> ") || current_str.ends_with("| ") {
                        output_lines.push(current_str.to_string());
                        break;
                    }
                    if byte[0] == b'\n' {
                        output_lines.push(current_str.to_string());
                        buffer.clear();
                    }
                }
                Err(e) => return Err(NeoError::SubprocessError { 
                    command: "GHCi output reading".to_string(),
                    output: format!("Error reading GHCi output: {}", e)
                }.into()),
            }
        }
        
        Ok(output_lines)
    }

    pub async fn stop(mut self) -> miette::Result<()> {
        self.stdin.write_all(b":quit\n").await.ok();
        self.child.wait().await
            .map_err(|e| NeoError::SubprocessError { 
                command: ":quit".to_string(),
                output: format!("Failed to wait for GHCi to exit: {}", e)
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[tokio::test]
    async fn test_read_until_prompt() {
        let input = "GHCi, version 9.4.5: https://www.haskell.org/ghc/  :? for help\n[1 of 1] Compiling Main             ( src/Main.hs, interpreted )\nOk, one module loaded.\nghci> ";
        let mut cursor = Cursor::new(input);
        
        let output = GhciSession::read_until_prompt(&mut cursor).await.unwrap();
        assert_eq!(output.len(), 4);
        assert_eq!(output[3], "ghci> ");
    }

    #[tokio::test]
    async fn test_read_until_prompt_multiline() {
        let input = "Some output\nMore output\nPrelude> ";
        let mut cursor = Cursor::new(input);
        
        let output = GhciSession::read_until_prompt(&mut cursor).await.unwrap();
        assert_eq!(output.len(), 3);
        assert_eq!(output[0], "Some output\n");
        assert_eq!(output[1], "More output\n");
        assert_eq!(output[2], "Prelude> ");
    }

    #[tokio::test]
    async fn test_read_until_prompt_with_pipe() {
        let input = "module Main where\n  | ";
        let mut cursor = Cursor::new(input);
        
        let output = GhciSession::read_until_prompt(&mut cursor).await.unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(output[1], "  | ");
    }

    #[tokio::test]
    async fn test_read_until_prompt_empty() {
        let input = "> ";
        let mut cursor = Cursor::new(input);
        
        let output = GhciSession::read_until_prompt(&mut cursor).await.unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0], "> ");
    }

    #[tokio::test]
    async fn test_read_until_prompt_eof_error() {
        let input = "Some output without prompt";
        let mut cursor = Cursor::new(input);
        
        let result = GhciSession::read_until_prompt(&mut cursor).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Error reading GHCi output"));
    }
}
