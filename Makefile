# Minimal makefile for Sphinx documentation
#

# You can set these variables from the command line, and also
# from the environment for the first two.
SPHINXOPTS    ?=
SPHINXBUILD   ?= .venv/bin/sphinx-build
SOURCEDIR     = source
BUILDDIR      = build

.PHONY: docs
docs:
	$(SPHINXBUILD) -M clean "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)
	$(SPHINXBUILD) -M html "$(SOURCEDIR)" "$(BUILDDIR)" $(SPHINXOPTS)

	rm -rf docs/*
	touch docs/.nojekyll
	mv build/html/* docs/
	cp -rv cla/ docs/