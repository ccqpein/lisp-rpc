;;; Let 's say I receive the pure lisp-rpc data

;;; and I define the function `hello-world`
(defun hello-world (&rest args &key my-name from)
  (declare (ignore args))
  (format nil "Hello~@[ to ~A~]!~@[ Friend from ~A!~]" my-name from))


;; Then run code below
(let ((pure-data-0 "(hello-world)")
      (pure-data-1 "(hello-world :from \"USA\")")
      (pure-data-2 "(hello-world :from \"Japan\" :my-name \"Mikasa\")"))
  (pprint (eval (read-from-string pure-data-0)))
  (pprint (eval (read-from-string pure-data-1)))
  (pprint (eval (read-from-string pure-data-2))))
;; The results are:
;; "Hello!"
;; "Hello! Friend from USA!"
;; "Hello to Mikasa! Friend from Japan!"
