(ql:quickload '("str" "alexandria" "fiveam"))

(defpackage test-lisp-rpc
  (:use #:cl #:lisp-rpc-checker)
  (:import-from #:fiveam
                #:def-suite
                #:in-suite
                #:test
                #:is
                #:signals
                #:def-fixture
                #:with-fixture
                #:run-all-tests
                #:run!)
  (:export #:test-lisp-rpc))

(in-package :test-lisp-rpc)

(def-suite test-lisp-rpc
  :description "Top level test suite")

(in-suite test-lisp-rpc)

(test list-type-checker-test
  (is (lisp-rpc-checker::list-type-checker 'list 'number))
  (is (not (lisp-rpc-checker::list-type-checker)))
  (is (not (lisp-rpc-checker::list-type-checker 'failed 'a)))
  (is (not (lisp-rpc-checker::list-type-checker 'list '()))))

(test map-data-type-checker-test
  (signals error (lisp-rpc-checker::map-data-type-checker '(:a 1)))
  (is (lisp-rpc-checker::map-data-type-checker '(:a 'string :b 'number)))
  (is (lisp-rpc-checker::map-data-type-checker '(:a '(:aa 'string))))
  (is (lisp-rpc-checker::map-data-type-checker '(:a '(list 'string))))
  (is (lisp-rpc-checker::map-data-type-checker
       '(:title 'string :version 'string :lang '(:lang 'string :encoding 'number)))))

(test def-msg-format-checker-test
  (is (not (lisp-rpc-checker::def-msg-checker "a" '(:a 'string :a '()))))
  (signals error (lisp-rpc-checker::def-msg-checker "a" '(:a 'string :a '(:a 1))))
  
  (is (lisp-rpc-checker::spec-check-one (read (make-string-input-stream "(def-msg language-perfer :lang 'string)"))))
  (is (lisp-rpc-checker::spec-check-one (read (make-string-input-stream "(def-msg language-perfers :langs '(list 'string))"))))
  (is (lisp-rpc-checker::spec-check-one (read (make-string-input-stream "(def-msg user :name '(:first 'string :second 'string))"))))

  ;; for def, list doesn't need to be quoted
  (is (lisp-rpc-checker::spec-check-one (read (make-string-input-stream "(def-msg user :name (:first 'string :second 'string))")))))

(test def-rpc-format-checker-test
  (is (lisp-rpc-checker::def-rpc-checker "get-book"
          '('(:title 'string :vesion 'string :lang '(:lang 'string :encoding 'number))
            'book-info)))
  
  (is (lisp-rpc-checker::spec-check-one (read (make-string-input-stream "(def-rpc get-book
    '(:title 'string :vesion 'string :lang '(:lang 'string :encoding 'number))
  'book-info)"))))
  (signals error (lisp-rpc-checker::spec-check-one (read (make-string-input-stream "(def-rpc get-book
    '(:title 'string :vesion 'string :lang '(:lang 'string :encoding 'number))
  1)"))))

  (is (lisp-rpc-checker::spec-check-one (read (make-string-input-stream "(def-rpc get-book
    (:title 'string :vesion 'string :lang (:lang 'string :encoding 'number))
  'book-info)")))))
