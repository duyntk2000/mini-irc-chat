Encryption Scheme
-Client and Server generate pair of private and public key
-Client send request to use secure communication with their public key
-Server make combined key with their private key and response with their public key
-Client make combined key
-Client (or can be Server) generate a shared key and encrypt it with combined key and send it
-Server (or can be Client) decrypt the shared key
-Communication is now encrypted/decrypted by that shared key

Pros:
-A type of hybrid encryption, fast encryption/decryption and secure share key process

Cons:
-Vulnerable to Man in the Middle