@testable import CmuxNextActions
import CmuxNextCompat
@testable import CmuxNextDaemonService
import Observation
import Testing

/// Leo, 2026-11-17: "remove bullshit ceremony + bullshit text + bullshit belt and suspenders". What a person does
/// while cmux is still starting waits and runs once it is ready, in order, instead of "cmux is still starting its
/// background service. Try again in a moment."; only a start that never comes shows one line
/// (``DaemonStartupWait``). The CLI, MCP and scripts are refused with the reason as before.
@MainActor
@Suite(.timeLimit(.minutes(1)))
struct DaemonStartupWaitTests {
    /// An action off while cmux starts (its availability says so) and one whose handler refuses then.
    static let offWhileStarting: ActionID = "cloudOpenDesktop"
    static let refusesWhileStarting: ActionID = "openSettings"

    @Observable @MainActor final class Gate { var readiness = DaemonStartupWait.Readiness.waiting }
    @Observable @MainActor final class Seen { var ran: [ActionID] = []; var refused: [String] = [] }

    struct Setup {
        let registry: ActionRegistry
        let gate: Gate
        let seen: Seen
        let wait: DaemonStartupWait
    }

    static func setup(patience: Duration = .seconds(600)) -> Setup {
        let registry = ActionRegistry.standard()
        registry.context = ActionContext(rawValue: .max)
        let gate = Gate(), seen = Seen()
        registry.refusalObserver = { reason, _ in seen.refused.append(reason) }
        registry.bind(offWhileStarting, unavailable: { gate.readiness == .ready ? nil : RefusalStrings.daemonConnecting },
                      invoke: { _ in seen.ran.append(offWhileStarting) })
        registry.bind(refusesWhileStarting, unavailable: { nil }, invoke: { _ in
            guard gate.readiness == .ready else { return registry.refuse(RefusalStrings.personalStateLoading) }
            seen.ran.append(refusesWhileStarting)
        })
        let wait = DaemonStartupWait(readiness: { _ in gate.readiness }, patience: patience)
        wait.bind(into: registry)
        return Setup(registry: registry, gate: gate, seen: seen, wait: wait)
    }

    static func until(_ done: @escaping @MainActor @Sendable () -> Bool) async {
        for await met in ObservationStream(done) where met { return }
    }

    @Test func runsWhileCmuxStartsRunOnceItIsReadyInOrder() async {
        let s = Self.setup()
        #expect(s.registry.perform(Self.offWhileStarting, invocation: ActionInvocation()))
        s.registry.perform(Self.refusesWhileStarting, invocation: ActionInvocation())
        s.registry.perform(Self.offWhileStarting, invocation: ActionInvocation())
        #expect(s.seen.ran.isEmpty, "a run went before cmux was ready")
        #expect(s.seen.refused.isEmpty, "a run was refused instead of waiting: \(s.seen.refused)")
        #expect(s.wait.waitingCount == 3)
        s.gate.readiness = .ready
        let seen = s.seen
        await Self.until { seen.ran.count == 3 }
        #expect(seen.ran == [Self.offWhileStarting, Self.refusesWhileStarting, Self.offWhileStarting])
        #expect(seen.refused.isEmpty, "something was shown: \(seen.refused)")
        #expect(s.wait.waitingCount == 0)
    }

    @Test func aStartThatNeverComesShowsOneLineAndRunsNothing() async {
        let s = Self.setup(patience: .milliseconds(50))
        s.registry.perform(Self.offWhileStarting, invocation: ActionInvocation())
        s.registry.perform(Self.refusesWhileStarting, invocation: ActionInvocation())
        let seen = s.seen
        await Self.until { !seen.refused.isEmpty }
        #expect(seen.refused == [RefusalStrings.startupWaitDropped])
        #expect(s.wait.waitingCount == 0)
        s.gate.readiness = .ready
        await Task.yield()
        #expect(seen.ran.isEmpty, "a dropped run ran later")
    }

    @Test func aServiceThatFailsToStartShowsOneLine() async {
        let s = Self.setup()
        s.registry.perform(Self.offWhileStarting, invocation: ActionInvocation())
        s.registry.perform(Self.offWhileStarting, invocation: ActionInvocation())
        s.gate.readiness = .failed
        let seen = s.seen
        await Self.until { !seen.refused.isEmpty }
        #expect(seen.refused == [RefusalStrings.startupWaitDropped], "one line, not one per run")
        #expect(seen.ran.isEmpty)
    }

    @Test func theCLIAndScriptsAreRefusedWithTheReasonAsBefore() {
        let s = Self.setup()
        for origin in [ActionOrigin.cli, .mcp, .script] {
            #expect(!s.registry.perform(Self.offWhileStarting, invocation: ActionInvocation(origin: origin)))
            let refused = s.registry.capturingRefusal {
                s.registry.perform(Self.refusesWhileStarting, invocation: ActionInvocation(origin: origin))
            }
            #expect(refused == RefusalStrings.personalStateLoading)
        }
        #expect(s.wait.waitingCount == 0)
    }

    /// A reason that does not pass by itself is refused as before.
    @Test func otherReasonsAreNotHeld() {
        #expect(DaemonStartupWait.waits(for: RefusalStrings.daemonConnecting))
        #expect(DaemonStartupWait.waits(for: RefusalStrings.personalStateLoading))
        #expect(!DaemonStartupWait.waits(for: RefusalStrings.daemonUnavailable))
        #expect(!DaemonStartupWait.waits(for: RefusalStrings.notInThisVersion))
    }
}
