test:
	RUST_LOG=thread_forever=info cargo test -v ${TEST} -- --nocapture
