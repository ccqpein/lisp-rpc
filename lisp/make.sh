#!/usr/bin/env bash
sbcl --load lisp/lisp-rpc.asd --eval '(ql:quickload :lisp-rpc)' --eval '(asdf:make :lisp-rpc)' --eval '(quit)'
