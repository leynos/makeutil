# A conditional assignment kept out of recipe environments by `unexport`,
# then re-exported under another name through `$(value ...)`, as a Makefile
# does to stop a tool reading a variable whose name it also recognizes.
TOOL_SOURCE ?= git+https://example.com/tool@0123456789abcdef
TOOL ?= uv tool run --from $(TOOL_SOURCE) tool
DOC_FLAGS ?= --cfg docsrs -D warnings
unexport DOC_FLAGS
export DOCFLAGS := $(value DOC_FLAGS)
CHECK_FLAGS ?=

docs: ## Build the documentation
	cargo doc --no-deps
