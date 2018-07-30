//
//  Protocol.swift
//  ced
//
//  Created by Martin Chaine on 19/06/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Foundation

enum RpcDataError: Error {
    case casting(String)
    case decoding(String)
}

enum RpcId: Codable, Hashable {
    case int(Int)
    case string(String)
    
    init(from value: Any) throws {
        if let v = value as? Int {
            self = .int(v)
        } else if let v = value as? String {
            self = .string(v)
        } else {
            throw RpcDataError.decoding("invalid id \(value)")
        }
    }
    
    init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer()
        if let v = try? value.decode(Int.self) {
            self = .int(v)
            return
        }
        if let v = try? value.decode(String.self) {
            self = .string(v)
            return
        }
        throw RpcDataError.decoding("invalid id \(dump(value))")
    }
    
    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(self)
    }
}

protocol RpcMessage {
    var jsonrpc: String { get }
}

extension RpcMessage {
    var jsonrpc: String { return "2.0" }
}

struct RpcRequest: RpcMessage {
    let method: String
    let params: Any?
    let id: RpcId?
    
    init(id: RpcId, method: String, params: Any? = nil) {
        self.id = id
        self.method = method
        self.params = params
    }
    
    init(with raw: Data) throws {
        guard let payload = try? JSONSerialization.jsonObject(with: raw) as! [String: Any] else {
            throw RpcDataError.decoding("invalid data \(raw)")
        }
        if let method = payload["method"] {
            self.method = method as! String
        } else {
            throw RpcDataError.decoding("missing attribute \"method\"")
        }
        self.params = payload["params"]
        if let id = payload["id"] {
            self.id = try RpcId(from: id)
        } else {
            self.id = nil
        }
    }
    
    func toDict() -> [String: Any] {
        var data: [String: Any] = [:]
        data["jsonrpc"] = self.jsonrpc
        if let id = self.id {
            switch id {
            case .int(let i): data["id"] = i
            case .string(let s): data["id"] = s
            }
        }
        data["method"] = self.method
        if self.params != nil {
            data["params"] = self.params
        }
        return data
    }
    
    func isNotification() -> Bool {
        return self.id != nil
    }
}

struct RpcError {
    let code: Int
    let message: String
    let data: Any?
}

struct RpcResponse: RpcMessage {
    let result: Any?
    let error: RpcError?
    let id: RpcId
    
    init(with raw: Data) throws {
        guard let payload = try? JSONSerialization.jsonObject(with: raw) as! [String: Any] else {
            throw RpcDataError.decoding("invalid data \(raw)")
        }
        self.result = payload["result"]
        if let error = payload["error"] {
            let e = error as! [String: Any]
            self.error = RpcError(code: e["code"] as! Int, message: e["message"] as! String, data: e["data"])
        } else {
            self.error = nil
        }
        self.id = try RpcId(from: payload["id"]!)
    }
    
    func isError() -> Bool {
        return self.error != nil
    }
}
