ROOT ?= root

build:
	cargo build

install:
	mkdir --parent "$(ROOT)/bin"
	mv --force target/*/ipconfig "$(ROOT)/bin"
	mv --force target/*/ping "$(ROOT)/bin"
	mv --force target/*/tracert "$(ROOT)/bin"
	mv --force target/*/pathping "$(ROOT)/bin"
	mv --force target/*/getmac "$(ROOT)/bin"
	mv --force target/*/fc "$(ROOT)/bin"
	mv --force target/*/findstr "$(ROOT)/bin"
	mv --force target/*/sort "$(ROOT)/bin"
	mv --force target/*/where "$(ROOT)/bin"
	mv --force target/*/expand "$(ROOT)/bin"
	mv --force target/*/makecab "$(ROOT)/bin"
	mv --force target/*/replace "$(ROOT)/bin"
	mv --force target/*/robocopy "$(ROOT)/bin"
	mv --force target/*/tree "$(ROOT)/bin"
	mv --force target/*/more "$(ROOT)/bin"
	mv --force target/*/type "$(ROOT)/bin"
	mv --force target/*/choice "$(ROOT)/bin"
	mv --force target/*/systeminfo "$(ROOT)/bin"
	mv --force target/*/tasklist "$(ROOT)/bin"
	mv --force target/*/taskkill "$(ROOT)/bin"
	mv --force target/*/shutdown "$(ROOT)/bin"
	mv --force target/*/whoami "$(ROOT)/bin"
	mv --force target/*/hostname "$(ROOT)/bin"
	mv --force target/*/ver "$(ROOT)/bin"
	mv --force target/*/clip "$(ROOT)/bin"

clean:
	rm --recursive --force target
