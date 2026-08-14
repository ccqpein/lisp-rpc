(defpackage lisp-rpc-generator
  (:use #:cl #:lisp-rpc-util)
  (:export #:def-msg
           #:def-rpc
           #:def-rpc-package
           #:generate-asd
           #:generate-project))

(in-package :lisp-rpc-generator)

(defun def-rpc-package (pkg-expression &optional stream)
  (unless (and (consp pkg-expression) (string-equal (symbol-name (first pkg-expression)) "def-rpc-package"))
    (error "first element of pkg-expression has to be def-rpc-package"))
  (let* ((name (second pkg-expression))
         (pkg (or (find-package name)
                  (make-package name :use '(#:cl #:lisp-rpc-util)))))
    (format stream "(defpackage ~a~%  (:use #:cl #:lisp-rpc-util))~%~%(in-package :~a)~%~%" name name)
    pkg))

(defun generate-asd (package-name &optional stream)
  "Generate ASDF system definition content for package-name."
  (let ((pkg-str (string-downcase (string package-name))))
    (format stream ";;;; -*- Mode: Lisp; Syntax: ANSI-Common-Lisp; Base: 10 -*-~%~%")
    (format stream "(defpackage :~a-sys~%  (:use :cl :asdf))~%~%" pkg-str)
    (format stream "(in-package :~a-sys)~%~%" pkg-str)
    (format stream "(defsystem :~a~%" pkg-str)
    (format stream "  :description \"Generated Lisp RPC System for ~a\"~%" pkg-str)
    (format stream "  :version \"0.0.1\"~%")
    (format stream "  :depends-on (\"lisp-rpc\")~%")
    (format stream "  :components ((:file \"lib\")))~%")))

(defun generate-project (spec-file-path output-dir-path)
  "Read spec file and generate a Lisp project directory containing .asd and lib.lisp."
  (ensure-directories-exist (uiop:ensure-directory-pathname output-dir-path))
  (let* ((raw-forms (uiop:read-file-forms spec-file-path))
         (pkg-form (find-if (lambda (x) (and (consp x) (symbolp (first x)) (string-equal (symbol-name (first x)) "DEF-RPC-PACKAGE"))) raw-forms))
         (package-name (if pkg-form (second pkg-form) "rpc-lib"))
         (pkg-str (string-downcase (string package-name)))
         (target-pkg (or (find-package package-name)
                         (make-package package-name :use '(#:cl #:lisp-rpc-util))))
         (forms (let ((*package* target-pkg))
                  (uiop:read-file-forms spec-file-path)))
         (asd-path (merge-pathnames (format nil "~a.asd" pkg-str) (uiop:ensure-directory-pathname output-dir-path)))
         (lib-path (merge-pathnames "lib.lisp" (uiop:ensure-directory-pathname output-dir-path))))
    
    (with-open-file (out asd-path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (generate-asd package-name out))
    
    (with-open-file (out lib-path :direction :output :if-exists :supersede :if-does-not-exist :create)
      (let ((*package* target-pkg))
        (dolist (form forms)
          (let ((head-name (when (and (consp form) (symbolp (first form)))
                             (symbol-name (first form)))))
            (cond
              ((string-equal head-name "def-rpc-package") (def-rpc-package form out))
              ((string-equal head-name "def-msg") (def-msg form out))
              ((string-equal head-name "def-rpc") (def-rpc form out))
              (t (error "Unknown spec form: ~S" form)))))))
    
    (values asd-path lib-path)))

(defun def-map-to-lisp-rpc-data (name keys &optional stream)
  (declare (ignore stream))
  (let* ((pkg (symbol-package name))
         (x (intern "X" pkg))
         (args (intern "ARGS" pkg)))
    `(defmethod to-lisp-rpc-data ((,x ,name) &rest ,args &key &allow-other-keys)
       (declare (ignore ,args))
       (format nil
               "(~{:~1{~A ~A~}~^ ~})"
               (list ,@(loop for k in keys
                             collect `(list (quote ,(intern (symbol-name k) pkg))
                                            (to-lisp-rpc-data (,(intern (format nil "~a-~a" name k) pkg) ,x)))))))))

(defun def-map (name map-kv-pairs &optional stream)
  "kind of the def-msg, but the defmethod define isn't same"
  (format stream "~a~%~%" (prin1-to-string (def-msg-struct name map-kv-pairs stream)))
  (format stream "~a~%~%" (prin1-to-string (def-map-to-lisp-rpc-data
                                               name
                                               (loop for (k a) on map-kv-pairs by #'cddr
                                                     collect k)))))

(defun def-list (name args &optional stream)
  "(list 'string), placeholder, no need to make the additional struct"
  (declare (ignore name args stream)))

(defun def-optional (name args &optional stream)
  "(optional 'string), placeholder, look like all fields in lisp struct is optional anyway"
  (declare (ignore name args stream)))

(defun def-funs-router (name key args)
  "pick which function should I call.
Return the function and the name of the type"
  (cond
    ((and (typep args 'cons) (keywordp (first args)))
     ;; the function and the new map struct name
     (list #'def-map (intern (format nil "~a-~a" name key) (symbol-package name))))
    
    ((and (typep args 'cons) (eq (first args) 'list))
     ;; it has to be the (list 'string), cannot missing the string's quote
     (list #'def-list `(cons ,(eval (second args)))))

    ((and (typep args 'cons) (eq (first args) 'optional))
     (list #'def-optional `(or null ,(eval (second args)))))
    
    (t (error "dont know which function should use for ~a" args))))

(defun def-msg (msg-expression &optional stream)
  (unless (and (consp msg-expression) (string-equal (symbol-name (first msg-expression)) "def-msg"))
    (error "first element of msg-expression has to be def-msg"))
  (format stream "~a~%~%" (prin1-to-string (def-msg-struct (second msg-expression) (cddr msg-expression) stream)))
  (format stream "~a~%~%" (prin1-to-string (def-msg-to-lisp-rpc-data
                                               (second msg-expression)
                                               (loop for (k a) on (cddr msg-expression) by #'cddr
                                                     collect k)))))

(defun def-msg-struct (name args &optional stream)
  (let ((pkg (symbol-package name)))
    `(defstruct ,name
       ,@(loop for (k tt) on args by #'cddr
               collect (let ((tt (if (and (consp tt) (eq (first tt) 'quote))
                                     (eval tt)
                                     tt)))
                         (cons (intern (string k) pkg)
                               (cond ((equal 'string tt)
                                      (list "" :type 'string))
                                     ((equal 'number tt)
                                      (list 0 :type 'number))
                                     ;; anonymous type
                                     ((typep tt 'cons)
                                      (let* ((router-result (def-funs-router name k tt)))
                                        ;;(format t "router-result: ~a~%" router-result)
                                        (funcall (first router-result) (second router-result) tt stream)
                                        (list nil :type (second router-result))))
                                     ;; default
                                     (t (list nil :type tt)))))))))

(defun def-msg-to-lisp-rpc-data (name keys &optional stream)
  (declare (ignore stream))
  (let* ((pkg (symbol-package name))
         (x (intern "X" pkg))
         (args (intern "ARGS" pkg)))
    `(defmethod to-lisp-rpc-data ((,x ,name) &rest ,args &key &allow-other-keys)
       (declare (ignore ,args))
       (format nil
               ,(format nil "(~a ~~{:~~1{~~A ~~A~~}~~^ ~~})" name)
               (list ,@(loop for k in keys
                             collect `(list (quote ,(intern (symbol-name k) pkg))
                                            (to-lisp-rpc-data (,(intern (format nil "~a-~a" name k) pkg) ,x)))))))))

(defun def-rpc (rpc-expression &optional stream)
  (unless (and (consp rpc-expression) (string-equal (symbol-name (first rpc-expression)) "def-rpc"))
    (error "first element of rpc-expression has to be def-rpc"))
  (let* ((name (second rpc-expression))
         (pkg (symbol-package name))
         (req-raw (third rpc-expression))
         (resp-raw (fourth rpc-expression))
         (req-args (if (and (consp req-raw) (eq (first req-raw) 'quote))
                       (eval req-raw)
                       req-raw))
         (resp-type (if (and (consp resp-raw) (eq (first resp-raw) 'quote))
                        (eval resp-raw)
                        resp-raw))
         (keys (loop for (k a) on req-args by #'cddr collect k))
         (req (intern "REQ" pkg)))
    (format stream "~a~%~%" (prin1-to-string (def-msg-struct name req-args stream)))
    (format stream "~a~%~%" (prin1-to-string (def-msg-to-lisp-rpc-data name keys stream)))
    (format stream "~a~%~%" (prin1-to-string `(defmethod rpc-endpoint-p ((,req ,name)) t)))
    (format stream "~a~%~%" (prin1-to-string `(defmethod rpc-response-type ((,req ,name)) ',resp-type)))))
