README.md: src/lib.rs
	echo "# dynec" > $@
	grep -P '^//!' $^ | cut -c5- | sed 's/^#/##/' >> $@
