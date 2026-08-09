;; (def-rpc-package demo)
(defpackage demo
  (:use #:cl)) ;; how to handle the use?

(in-package :demo)

;; (def-msg language-preference :lang 'string)
(defstruct language-preference
  (lang "" :type string))

(defmethod to-lisp-rpc-data ((x language-preference) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil
          "(language-preference ~{:~1{~A ~A~}~^ ~})"
          (list (list 'lang (to-lisp-rpc-data (language-preference-lang x))))))

;; (def-msg book-info
;;   :lang 'language-preference
;;   :title 'string
;;   :version 'string
;;   :id 'string)
(defstruct book-info
  (lang nil :type language-preference)
  (title "" :type string)
  (version "" :type string)
  (id "" :type string))

(defmethod to-lisp-rpc-data ((x book-info) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil
          "(book-info ~{:~1{~A ~A~}~^ ~})"
          (list (list 'lang (to-lisp-rpc-data (book-info-lang x)))
                (list 'title (to-lisp-rpc-data (book-info-title x)))
                (list 'version (to-lisp-rpc-data (book-info-version x)))
                (list 'id (to-lisp-rpc-data (book-info-id x))))))

(defstruct get-book-lang
  (lang "" :type string)
  (encoding 0 :type number))

(defmethod to-lisp-rpc-data ((x get-book-lang) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil
          "(~{:~1{~A ~A~}~^ ~})"
          (list (list 'lang (to-lisp-rpc-data (get-book-lang-lang x)))
                (list 'encoding (to-lisp-rpc-data (get-book-lang-encoding x))))))

;; (def-msg authors :names (list 'string))
(defstruct authors
  (names nil :type (cons string)))

(defmethod to-lisp-rpc-data ((x authors) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil
          "(authors ~{:~1{~A ~A~}~^ ~})"
          (list (list 'names (to-lisp-rpc-data (authors-names x))))))

;; (def-rpc get-book
;;     '(:title 'string :version 'string
;;       :lang '(:lang 'string :encoding 'number)
;;       :authors 'authors)
;;   'book-info)
(defstruct get-book
  (title "" :type string)
  (version "" :type string)
  (lang "" :type get-book-lang)
  (authors nil :type authors))

(defmethod to-lisp-rpc-data ((x get-book) &rest args &key &allow-other-keys)
  (declare (ignore args))
  (format nil
          "(get-book ~{:~1{~A ~A~}~^ ~})"
          (list (list 'title (to-lisp-rpc-data (get-book-title x)))
                (list 'version (to-lisp-rpc-data (get-book-version x)))
                (list 'lang (to-lisp-rpc-data (get-book-lang x)))
                (list 'authors (to-lisp-rpc-data (get-book-authors x))))))

;; get the function of the ops, 
(defmethod rpc-handle ((x get-book) func &rest args &key &allow-other-keys)
  )


(defun get-book (args handler)
  (declare ()) ;;:= declare the type that args is get-book-args, 
  )
