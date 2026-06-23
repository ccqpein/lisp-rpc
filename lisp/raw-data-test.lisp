(defpackage test-lisp-rpc-raw-data
  (:use #:cl #:lisp-rpc-raw-data)
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
  (:export #:test-raw-data))

(in-package :test-lisp-rpc-raw-data)

(def-suite test-raw-data
  :description "Test suite for raw-data")

(in-suite test-raw-data)

(test type-of-raw-data-test
  (is (eq :string (type-of-raw-data "a")))
  (is (eq :number (type-of-raw-data 1)))
  (is (eq :data (type-of-raw-data '(a :a 1 :b 2))))
  (is (eq :list (type-of-raw-data '(1 2 3 4))))
  (is (eq :map (type-of-raw-data '(:g 2 :3 4)))))

(test parse-data-test
  (let* ((parsed (parse-data "(update-user :id 1 :profile '(:email \"a@b.com\" :tags '(\"admin\" \"staff\")))"))
         (profile-map (data-get parsed :profile)))
    (is (raw-data-p parsed))
    (is (eq 'update-user (get-name parsed)))
    (is (= 1 (data-get parsed :id)))
    (is (raw-data-map-p profile-map))
    (is (equal '(:email "a@b.com" :tags '("admin" "staff")) (raw-data-map-kv profile-map)))
    (is (equal "a@b.com" (map-data-get profile-map :email)))))

(test parse-list-and-map-test
  (let ((parsed-list (parse-raw-data '(1 2 3))))
    (is (raw-data-list-p parsed-list))
    (is (equal '(1 2 3) (raw-data-list-l parsed-list))))
  (let ((parsed-map (parse-raw-data '(:a 1 :b 2))))
    (is (raw-data-map-p parsed-map))
    (is (equal '(:a 1 :b 2) (raw-data-map-kv parsed-map)))))

(test nested-raw-data-test
  (let* ((parsed (parse-data "(response-name :result (inner-data :val 123) :status \"success\")"))
         (result (data-get parsed :result)))
    (is (raw-data-p parsed))
    (is (eq 'response-name (get-name parsed)))
    (is (equal "success" (data-get parsed :status)))
    (is (raw-data-p result))
    (is (eq 'inner-data (get-name result)))
    (is (= 123 (data-get result :val)))))
