ROOT ?= root

build:
	cargo build

install:
	mkdir --parent "$(ROOT)/bin"
	mv target/*/ipconfig "$(ROOT)/bin"
	mv target/*/ping "$(ROOT)/bin"
	mv target/*/tracert "$(ROOT)/bin"
	mv target/*/pathping "$(ROOT)/bin"
	mv target/*/getmac "$(ROOT)/bin"
	mv target/*/fc "$(ROOT)/bin"
	mv target/*/findstr "$(ROOT)/bin"
	mv target/*/sort "$(ROOT)/bin"
	mv target/*/where "$(ROOT)/bin"
	mv target/*/expand "$(ROOT)/bin"
	mv target/*/makecab "$(ROOT)/bin"
	mv target/*/replace "$(ROOT)/bin"
	mv target/*/robocopy "$(ROOT)/bin"
	mv target/*/tree "$(ROOT)/bin"
	mv target/*/more "$(ROOT)/bin"
	mv target/*/type "$(ROOT)/bin"
	mv target/*/choice "$(ROOT)/bin"
	mv target/*/systeminfo "$(ROOT)/bin"
	mv target/*/tasklist "$(ROOT)/bin"
	mv target/*/taskkill "$(ROOT)/bin"
	mv target/*/shutdown "$(ROOT)/bin"
	mv target/*/whoami "$(ROOT)/bin"
	mv target/*/hostname "$(ROOT)/bin"
	mv target/*/ver "$(ROOT)/bin"
	mv target/*/clip "$(ROOT)/bin"

clean:
	rm --recursive --force target

