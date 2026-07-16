use std::{
    net::{TcpListener, TcpStream}, process::{Child, Command, Stdio},thread, time::{Duration, Instant},
};




pub struct AnvilInstance {
    process: Child,
    port: u16,

}

impl AnvilInstance {
    pub fn fork(
        rpc_url: &str,
        fork_block: Option<u64>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let port = find_free_port()?;

        let mut cmd = Command::new("anvil");

        cmd.arg("--fork-url")
            .arg(rpc_url)
            .arg("--port")
            .arg(port.to_string())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        if let Some(block) = fork_block {
            cmd.arg("--fork-block-number")
                .arg(block.to_string());
        }

        println!("Lancement d'Anvil sur le port {}", port);

        let mut process = cmd.spawn()?;

        let deadline = Instant::now() + Duration::from_secs(20);

        loop {
            // Si Anvil est mort, on récupère immédiatement le code de sortie.
            if let Some(status) = process.try_wait()? {
                return Err(format!("Anvil s'est arrêté immédiatement ({status})").into());
            }

            // Le serveur répond.
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                println!("Anvil prêt sur le port {}", port);

                return Ok(Self {
                    process,
                    port,
                });
            }

            if Instant::now() >= deadline {
                let _ = process.kill();
                return Err("Timeout : Anvil n'a pas démarré".into());
            }

            thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn ws_endpoint(&self) -> String {
        format!("ws://127.0.0.1:{}", self.port)
    }


 
  
}

impl Drop for AnvilInstance {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn find_free_port() -> Result<u16, Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    Ok(listener.local_addr()?.port())
}