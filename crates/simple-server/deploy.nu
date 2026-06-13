cargo zigbuild --release -p simple-server --target x86_64-unknown-linux-gnu
ssh felixb@($env.FERMI_IP) "sudo systemctl stop simple-server"
scp target/x86_64-unknown-linux-gnu/release/simple-server felixb@($env.FERMI_IP):/tmp/simple-server
ssh felixb@($env.FERMI_IP) "sudo systemctl stop simple-server && sudo mv /tmp/simple-server /usr/local/bin/simple-server && sudo restorecon /usr/local/bin/simple-server && sudo systemctl start simple-server"
