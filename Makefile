ROOT ?= root

build:
	cargo build --release

install:
	mkdir --parent "$(ROOT)/bin"
	mv --force target/release/ipconfig "$(ROOT)/bin"
	mv --force target/release/ping "$(ROOT)/bin"
	mv --force target/release/tracert "$(ROOT)/bin"
	mv --force target/release/pathping "$(ROOT)/bin"
	mv --force target/release/getmac "$(ROOT)/bin"
	mv --force target/release/fc "$(ROOT)/bin"
	mv --force target/release/findstr "$(ROOT)/bin"
	mv --force target/release/sort "$(ROOT)/bin"
	mv --force target/release/where "$(ROOT)/bin"
	mv --force target/release/expand "$(ROOT)/bin"
	mv --force target/release/makecab "$(ROOT)/bin"
	mv --force target/release/replace "$(ROOT)/bin"
	mv --force target/release/robocopy "$(ROOT)/bin"
	mv --force target/release/tree "$(ROOT)/bin"
	mv --force target/release/more "$(ROOT)/bin"
	mv --force target/release/type "$(ROOT)/bin"
	mv --force target/release/choice "$(ROOT)/bin"
	mv --force target/release/systeminfo "$(ROOT)/bin"
	mv --force target/release/tasklist "$(ROOT)/bin"
	mv --force target/release/taskkill "$(ROOT)/bin"
	mv --force target/release/shutdown "$(ROOT)/bin"
	mv --force target/release/whoami "$(ROOT)/bin"
	mv --force target/release/hostname "$(ROOT)/bin"
	mv --force target/release/ver "$(ROOT)/bin"
	mv --force target/release/clip "$(ROOT)/bin"

clean:
	rm --recursive --force target
