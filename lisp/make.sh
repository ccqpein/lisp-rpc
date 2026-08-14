#!/usr/bin/env bash
sbcl --load lisp-rpc.asd --eval '(ql:quickload :lisp-rpc)' --eval '(asdf:make :lisp-rpc)' --eval '(quit)'
