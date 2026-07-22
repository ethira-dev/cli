# Ethira CLI helpers
# https://github.com/casey/just

default:
    @just --list

# Docs / marketing site (GitHub Pages)
site-install:
    cd site && npm install

site-dev:
    cd site && npm run dev

site-build:
    cd site && npm run build
