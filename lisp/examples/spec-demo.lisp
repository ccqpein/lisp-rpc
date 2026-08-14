(def-rpc-package demo)

(def-msg language-preference :lang 'string)

(def-msg book-info
  :lang 'language-preference
  :title 'string
  :version 'string
  :id 'string)

(def-msg authors :names (list 'string))

(def-rpc get-book
    '(:title 'string :version 'string
      :lang '(:lang 'string :encoding 'number)
      :authors 'authors)
  'book-info)

(def-rpc ping-no-pong
    '(:nothing 'string))
