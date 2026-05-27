cargo zigbuild --release -p simple-server --target x86_64-unknown-linux-gnu
ssh felixb@fermi "sudo systemctl stop simple-server"
scp target/x86_64-unknown-linux-gnu/release/simple-server felixb@fermi:/tmp/simple-server
ssh felixb@fermi "sudo mv /tmp/simple-server /usr/local/bin/simple-server && sudo restorecon /usr/local/bin/simple-server && sudo systemctl start simple-server"
