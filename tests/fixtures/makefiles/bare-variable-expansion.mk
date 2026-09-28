# A bare $(VAR) line is parsed from its expanded text. Here it defines the
# rule `generated`, which a static parse cannot see, so the line becomes a
# diagnostic and the parse stays recovered.
GENERATED_RULE := generated: ; @echo generated
$(GENERATED_RULE)

build: generated
	@echo built
