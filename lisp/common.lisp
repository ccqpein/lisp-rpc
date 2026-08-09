(defpackage common
  (:use #:cl)
  (:export #:to-lisp-rpc-data
           #:from-lisp-rpc-data
           #:rpc-endpoint-p
           #:rpc-response-type))

(in-package :common)

(defgeneric rpc-endpoint-p (req)
  (:documentation "Returns T if REQ is a top-level callable RPC endpoint struct.")
  (:method (req) nil))

(defgeneric rpc-response-type (req)
  (:documentation "Returns the expected response type symbol for an RPC request struct.")
  (:method (req) nil))

(defmethod to-lisp-rpc-data ((x null) &rest args &key &allow-other-keys)
  (declare (ignore args))
  "")

(defmethod to-lisp-rpc-data ((x string) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil "~S" x))

(defmethod to-lisp-rpc-data ((x number) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil "~a" x))

(defmethod to-lisp-rpc-data ((x cons) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil "'(~{~a~^ ~})" (mapcar #'to-lisp-rpc-data x)))

(defun instantiate-struct (struct-name kv-pairs)
  (let* ((ctor (find-symbol (format nil "MAKE-~A" (symbol-name struct-name))
                            (symbol-package struct-name)))
         (parsed-args (loop for (k v) on kv-pairs by #'cddr
                            append (list k (from-lisp-rpc-data v)))))
    (handler-case
        (apply ctor parsed-args)
      (error ()
        (let* ((cls (find-class struct-name nil))
               (inst (when cls (allocate-instance cls))))
          (if (not inst)
              nil
              #+sbcl
              (let* ((dd (sb-kernel:find-defstruct-description struct-name))
                     (slots (when dd (sb-kernel:dd-slots dd))))
                (dolist (s slots)
                  (let* ((slot-kw (intern (symbol-name (sb-kernel:dsd-name s)) "KEYWORD"))
                         (val (getf parsed-args slot-kw))
                         (idx (sb-kernel:dsd-index s)))
                    (setf (sb-kernel:%instance-ref inst idx) val)))
                inst)
              #-sbcl
              (progn
                (loop for (k v) on parsed-args by #'cddr do
                  (let ((slot-name (intern (symbol-name k) (symbol-package struct-name))))
                    (ignore-errors (setf (slot-value inst slot-name) v))))
                inst)))))))

(defun from-lisp-rpc-data (data &rest args &key &allow-other-keys)
  (declare (ignore args))
  (cond
    ((null data) nil)
    ((stringp data)
     (let ((trimmed (string-trim '(#\Space #\Tab #\Newline #\Return) data)))
       (if (string= trimmed "")
           nil
           (let ((parsed (read-from-string trimmed)))
             (cond
               ((null parsed) nil)
               ((or (stringp parsed) (numberp parsed)) parsed)
               ((consp parsed) (from-lisp-rpc-data parsed))
               (t data))))))
    ((consp data)
     (cond
       ;; Unquote '(...)
       ((eq (first data) 'quote)
        (from-lisp-rpc-data (second data)))

       ;; Struct pattern: (struct-name ...) or (struct-name :key1 val1 ...)
       ((and (symbolp (first data))
             (let ((ctor (find-symbol (format nil "MAKE-~A" (symbol-name (first data)))
                                      (symbol-package (first data)))))
               (and ctor (fboundp ctor))))
        (instantiate-struct (first data) (cdr data)))

       ;; Map / Plist pattern: (:key1 val1 :key2 val2)
       ((keywordp (first data))
        (loop for (k v) on data by #'cddr
              append (list k (from-lisp-rpc-data v))))

       ;; Standard list: (elem1 elem2 ...)
       (t
        (mapcar #'from-lisp-rpc-data data))))
    (t data)))
