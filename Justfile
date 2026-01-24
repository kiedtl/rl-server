deploy:
	cargo build --release
	ssh illuvatar -- 'sudo systemctl stop oathbreaker'
	scp target/release/oathbreaker-server illuvatar:/home/oathbreaker/server/oathd
	scp -r public illuvatar:/home/oathbreaker/server/
	scp -r content illuvatar:/home/oathbreaker/server/
	ssh illuvatar -- 'sudo systemctl start oathbreaker'
