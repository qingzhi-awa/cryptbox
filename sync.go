package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"
	"time"
)

// SyncClient 对接同步服务端（passbook/server）。
type SyncClient struct {
	BaseURL string
	Token   string
	http    *http.Client
}

func NewSyncClient(baseURL string) *SyncClient {
	return &SyncClient{
		BaseURL: strings.TrimRight(baseURL, "/"),
		http:    &http.Client{Timeout: 15 * time.Second},
	}
}

// AuthResult 登录/注册返回结果。
type AuthResult struct {
	Token    string
	Username string
	Avatar   string
}

func (c *SyncClient) Register(username, password, email, code string) (AuthResult, error) {
	body, _ := json.Marshal(map[string]string{
		"username": username,
		"password": password,
		"email":    email,
		"code":     code,
	})
	return c.auth("/api/register", body)
}

func (c *SyncClient) Login(username, password string) (AuthResult, error) {
	body, _ := json.Marshal(map[string]string{"username": username, "password": password})
	return c.auth("/api/login", body)
}

func (c *SyncClient) auth(path string, body []byte) (AuthResult, error) {
	resp, err := c.http.Post(c.BaseURL+path, "application/json", bytes.NewReader(body))
	if err != nil {
		return AuthResult{}, err
	}
	defer resp.Body.Close()

	var out struct {
		Token    string `json:"token"`
		Username string `json:"username"`
		Avatar   string `json:"avatar"`
		Error    string `json:"error"`
	}
	b, _ := io.ReadAll(resp.Body)
	_ = json.Unmarshal(b, &out)
	if resp.StatusCode != http.StatusOK {
		if out.Error != "" {
			return AuthResult{}, errors.New(out.Error)
		}
		return AuthResult{}, fmt.Errorf("请求失败: %d", resp.StatusCode)
	}
	return AuthResult{Token: out.Token, Username: out.Username, Avatar: out.Avatar}, nil
}

// SendRegisterCode 请求服务端发送注册验证码。
func (c *SyncClient) SendRegisterCode(email string) error {
	body, _ := json.Marshal(map[string]string{"email": email})
	resp, err := c.http.Post(c.BaseURL+"/api/register/send-code", "application/json", bytes.NewReader(body))
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		var out struct {
			Error string `json:"error"`
		}
		b, _ := io.ReadAll(resp.Body)
		_ = json.Unmarshal(b, &out)
		if out.Error != "" {
			return errors.New(out.Error)
		}
		return fmt.Errorf("请求失败: %d", resp.StatusCode)
	}
	return nil
}

// Check 校验 token 是否有效（调用服务端 /api/me）。
func (c *SyncClient) Check() error {
	req, _ := http.NewRequest(http.MethodGet, c.BaseURL+"/api/me", nil)
	req.Header.Set("Authorization", "Bearer "+c.Token)
	resp, err := c.http.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("token 无效: %d", resp.StatusCode)
	}
	return nil
}

// Push 将本地密码条目整体上传，替换服务端该账号的密码库。
func (c *SyncClient) Push(entries []Entry) error {
	body, _ := json.Marshal(map[string]interface{}{"entries": entries})
	req, _ := http.NewRequest(http.MethodPut, c.BaseURL+"/api/vault", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+c.Token)
	resp, err := c.http.Do(req)
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("推送失败: %d", resp.StatusCode)
	}
	return nil
}

// Pull 下载服务端该账号的密码条目。
func (c *SyncClient) Pull() ([]Entry, error) {
	req, _ := http.NewRequest(http.MethodGet, c.BaseURL+"/api/vault", nil)
	req.Header.Set("Authorization", "Bearer "+c.Token)
	resp, err := c.http.Do(req)
	if err != nil {
		return nil, err
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("下载失败: %d", resp.StatusCode)
	}
	var out struct {
		Entries []Entry `json:"entries"`
	}
	b, _ := io.ReadAll(resp.Body)
	_ = json.Unmarshal(b, &out)
	return out.Entries, nil
}
