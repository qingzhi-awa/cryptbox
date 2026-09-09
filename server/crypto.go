package main

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/rand"
	"database/sql"
	"encoding/base64"
	"errors"
)

// aesEncrypt 使用 AES-256-GCM 加密，返回 nonce+ciphertext 的 base64。
func aesEncrypt(key, plaintext []byte) (string, error) {
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

func aesDecrypt(key []byte, encoded string) ([]byte, error) {
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

func aesEncryptString(key []byte, s string) (string, error) {
	return aesEncrypt(key, []byte(s))
}

func aesDecryptString(key []byte, encoded string) (string, error) {
	b, err := aesDecrypt(key, encoded)
	if err != nil {
		return "", err
	}
	return string(b), nil
}

// getOrCreateEncryptionKey 读取或生成服务端静态加密密钥（持久化在 meta 表）。
func getOrCreateEncryptionKey(db *sql.DB) ([]byte, error) {
	keyB64 := getMeta(db, "encryption_key")
	if keyB64 != "" {
		return base64.StdEncoding.DecodeString(keyB64)
	}
	key := make([]byte, 32)
	if _, err := rand.Read(key); err != nil {
		return nil, err
	}
	if err := setMeta(db, "encryption_key", base64.StdEncoding.EncodeToString(key)); err != nil {
		return nil, err
	}
	return key, nil
}
