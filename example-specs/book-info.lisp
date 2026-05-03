;;; define the spec of the api
;;; this should be optional

(def-rpc-package demo)

(def-msg language-perfer :lang 'string)

(def-msg book-info
  :lang 'language-perfer
  :title 'string
  :version 'string
  :id 'string)

(def-rpc get-book
    '(:title 'string :version 'string
      :lang '(:lang 'string :encoding 'number)
      :authors 'authors)
  'book-info)

(def-msg authors :names (list 'string))
