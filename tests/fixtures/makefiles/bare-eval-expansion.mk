# $(eval ...) and a $(foreach ...) of $(eval $(call ...)) can define rules
# and variables that a static parse cannot see, so each line becomes a
# diagnostic and the parse stays recovered.
define make-rule
$(1):
	@echo $(1)
endef
NAMES := alpha beta
$(eval EXTRA := gamma)
$(foreach name,$(NAMES),$(eval $(call make-rule,$(name))))

build: alpha beta
	@echo $(EXTRA)
