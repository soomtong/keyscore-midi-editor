// CI 시험 단계 동안 CoreMIDI 클라이언트 하나를 쥐고 있어 MIDI 서버가 내려가지 않게 한다.
//
// MIDIServices.h: 마지막 클라이언트가 사라지면 MIDI 서버가 끝날 수 있다. 시험 바이너리는 저마다
// 프로세스라 바이너리 사이에 서버가 내려갈 수 있다. 원격 CI 실행 4에서 inspector의 첫 MIDIClientCreate가
// 10초 재시도(CLIENT_CREATE_PATIENCE) 동안 -50으로 실패했고, 「서버 재기동이 가끔 길어진다」를 가설로
// 본다(확인하지 못함, 반대 근거는 docs/reports/ci/first-run.md). 이 클라이언트는 그 경로를 막는다.
import CoreMIDI
import Foundation

var client = MIDIClientRef()
let status = MIDIClientCreate("keyscore-ci-keepalive" as CFString, nil, nil, &client)
guard status == noErr else {
    FileHandle.standardError.write("MIDIClientCreate \(status)\n".data(using: .utf8)!)
    exit(1)
}
print("ready")
fflush(stdout)
// RunLoop.main.run()은 입력 소스가 없으면 돌아올 수 있다고 문서화돼 있어 조건 없이 막는다(리뷰 1 지적 1).
// 실제 러너에서는 돌아오지 않았다(실행 41442088403, 리뷰 2 지적 2).
while true {
    sleep(3600)
}
