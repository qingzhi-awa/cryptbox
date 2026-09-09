package main

import (
	"fmt"
	"io"
	"net"
	"net/http"
	"strings"
	"sync"
	"time"
)

// syncPort 是飞牛服务端（同步服务器）的默认端口。
const syncPort = 5201

// ScanLAN 扫描局域网内可用的 CryPtBox 同步服务器，返回形如 http://ip:端口 的地址列表。
func (a *App) ScanLAN() ([]string, error) {
	ips := localIPv4s()
	if len(ips) == 0 {
		return []string{}, fmt.Errorf("未检测到局域网地址")
	}

	// 收集所有待扫描的主机（按 /24 网段去重，仅扫描私有网段）
	hostSet := map[string]struct{}{}
	for _, ip := range ips {
		if !isPrivateIPv4(ip) {
			continue
		}
		prefix := ip.Mask(net.CIDRMask(24, 32)).To4()
		if prefix == nil {
			continue
		}
		for i := 1; i <= 254; i++ {
			hostSet[fmt.Sprintf("%d.%d.%d.%d", prefix[0], prefix[1], prefix[2], i)] = struct{}{}
		}
	}

	client := &http.Client{Timeout: 400 * time.Millisecond}
	results := []string{}
	var (
		mu  sync.Mutex
		wg  sync.WaitGroup
		sem = make(chan struct{}, 128)
	)
	for host := range hostSet {
		wg.Add(1)
		go func(h string) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()

			url := fmt.Sprintf("http://%s:%d/api/health", h, syncPort)
			req, err := http.NewRequest(http.MethodGet, url, nil)
			if err != nil {
				return
			}
			resp, err := client.Do(req)
			if err != nil {
				return
			}
			defer resp.Body.Close()
			if resp.StatusCode != http.StatusOK {
				return
			}
			body, _ := io.ReadAll(io.LimitReader(resp.Body, 64))
			if !strings.Contains(string(body), "ok") {
				return
			}
			mu.Lock()
			results = append(results, fmt.Sprintf("http://%s:%d", h, syncPort))
			mu.Unlock()
		}(host)
	}
	wg.Wait()
	return results, nil
}

// localIPv4s 返回本机所有非回环的 IPv4 地址。
func localIPv4s() []net.IP {
	var out []net.IP
	addrs, err := net.InterfaceAddrs()
	if err != nil {
		return out
	}
	for _, a := range addrs {
		ipnet, ok := a.(*net.IPNet)
		if !ok || ipnet.IP.IsLoopback() {
			continue
		}
		if ip4 := ipnet.IP.To4(); ip4 != nil {
			out = append(out, ip4)
		}
	}
	return out
}

// isPrivateIPv4 判断是否为 RFC1918 私有网段（10.x / 172.16-31.x / 192.168.x）。
func isPrivateIPv4(ip net.IP) bool {
	ip4 := ip.To4()
	if ip4 == nil {
		return false
	}
	return ip4[0] == 10 ||
		(ip4[0] == 172 && ip4[1] >= 16 && ip4[1] <= 31) ||
		(ip4[0] == 192 && ip4[1] == 168)
}
