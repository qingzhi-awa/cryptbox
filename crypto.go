package main

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"encoding/base64"
	"errors"

	"golang.org/x/crypto/scrypt"
)

// deriveKey 使用 scrypt 从主密码派生 32 字节 AES-256 密钥。
func deriveKey(password string, salt []byte) ([]byte, error) {
	return scrypt.Key([]byte(password), salt, 32768, 8, 1, 32)
}

func newSalt() ([]byte, error) {
	s := make([]byte, 16)
	if _, err := rand.Read(s); err != nil {
		return nil, err
	}
	return s, nil
}

// encryptBytes 使用 AES-256-GCM 加密，返回 nonce+ciphertext 的 base64。
func encryptBytes(key, plaintext []byte) (string, error) {
	block, err := aes.NewCipher(key)
	if err != nil {
		return "", err
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return "", err
	}
	nonce := make([]byte, gcm.NonceSize())
	if _, err := rand.Read(nonce); err != nil {
		return "", err
	}
	ct := gcm.Seal(nonce, nonce, plaintext, nil)
	return base64.StdEncoding.EncodeToString(ct), nil
}

func decryptBytes(key []byte, encoded string) ([]byte, error) {
	ct, err := base64.StdEncoding.DecodeString(encoded)
	if err != nil {
		return nil, err
	}
	block, err := aes.NewCipher(key)
	if err != nil {
		return nil, err
	}
	gcm, err := cipher.NewGCM(block)
	if err != nil {
		return nil, err
	}
	if len(ct) < gcm.NonceSize() {
		return nil, errors.New("ciphertext too short")
	}
	nonce, ct := ct[:gcm.NonceSize()], ct[gcm.NonceSize():]
	return gcm.Open(nil, nonce, ct, nil)
}

func encryptString(key []byte, s string) (string, error) {
	return encryptBytes(key, []byte(s))
}

func decryptString(key []byte, encoded string) (string, error) {
	b, err := decryptBytes(key, encoded)
	if err != nil {
		return "", err
	}
	return string(b), nil
}
