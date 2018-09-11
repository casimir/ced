//
//  Ced.swift
//  ced
//
//  Created by Martin Chaine on 18/06/2018.
//  Copyright © 2018 Casimir Lab. All rights reserved.
//

import Foundation

var standardError = FileHandle.standardError

extension FileHandle : TextOutputStream {
    
    public func write(_ string: String) {
        guard let data = string.data(using: .utf8) else { return }
        self.write(data)
    }
    
}

class CedContext {
    
    var session: String!
    var currentBuffer: String!
    var bufferList: [String: [String: String]] = [:]
    
    func buffer() -> [String: String] {
        if let buffer = self.bufferList[self.currentBuffer] {
            return buffer
        } else {
            return ["content": ""]
        }
    }
    
}

class Ced {

    var handler: ConnectionController
    var context: CedContext
    
    var proc: Process
    var pipeIn: Pipe
    var pipeErr: Pipe
    var bufIn: Data
    var next_rpc_id = 1
    
    init(handler: ConnectionController) {
        self.context = CedContext()
        self.handler = handler
        
        self.proc = Process()
        self.pipeIn = Pipe()
        self.pipeErr = Pipe()
        self.pipeErr.fileHandleForReading.readabilityHandler = { handle in
            let message = String(data: handle.readDataToEndOfFile(), encoding: .utf8)!
            print(message, to: &standardError)
        }
        self.bufIn = Data(capacity: 65536)
    }
    
    func consumeData(data: Data) {
        let offset = self.bufIn.count
        self.bufIn.append(data)
        let bufLen = self.bufIn.count
        
        self.bufIn.withUnsafeMutableBytes({(bufInBytes: UnsafeMutablePointer<UInt8>) -> Void in
            var cursor = 0
            for i in offset ..< bufLen {
                if self.bufIn[i] != UInt8(ascii: "\n") {
                    continue
                }
                
                let line = self.bufIn.subdata(in: Range(cursor ..< i + 1))
                self.handler.handle(line: line, context: self.context)
                cursor = i + 1
            }
            if cursor < bufLen {
                memmove(bufInBytes, bufInBytes + cursor, bufLen - cursor)
            }
            self.bufIn.count = bufLen - cursor
        })
    }
    
    func connect(session: String? = nil) {
        var args: [String] = ["--mode=json"]
        if let name = session {
                args += ["--session=\(name)"]
        }
        
        let pipeOut = Pipe()
        pipeOut.fileHandleForReading.readabilityHandler = { handle in
            let data = handle.availableData
            if data.count > 0 {
                self.consumeData(data: data)
            }
        }
        
        self.proc.launchPath = "/Users/casimir/.cargo/bin/ced"
        self.proc.arguments = args
        self.proc.standardInput = self.pipeIn
        self.proc.standardOutput = pipeOut
        self.proc.standardError = self.pipeErr
        
        self.proc.launch()
    }
    
    func close() {
        self.proc.terminate()
    }
    
    func request(method: String, params: Any? = nil) {
        let request = RpcRequest(id: .int(self.next_rpc_id), method: method, params: params)
        if var payload = try? JSONSerialization.data(withJSONObject: request.toDict()) {
            payload.append(Data("\n".utf8))
            self.pipeIn.fileHandleForWriting.write(payload)
            self.handler.runningRequests[request.id!] = request
            self.next_rpc_id += 1
        } else {
            print("invalid request: \(request)")
        }
    }
    
    class func listSessions() -> [String] {
        let task = Process()
        task.launchPath = "/Users/casimir/.cargo/bin/ced"
        task.arguments = ["-l"]
        let pipe = Pipe()
        task.standardOutput = pipe
        task.launch()
        task.waitUntilExit()
        
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        let output = String(data: data, encoding: .utf8)!.trimmingCharacters(in: CharacterSet.newlines)
        let sessions = output.components(separatedBy: CharacterSet.newlines)
        return sessions.sorted()
    }

}
