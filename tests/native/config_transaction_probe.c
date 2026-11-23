#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

#if defined(__APPLE__) && defined(__arm64__)
#include <sys/mount.h>
#define FIXTURE_FILESYSTEM "apfs"
#define FIXTURE_ARCHITECTURE "arm64"
#elif defined(__linux__) && defined(__x86_64__)
#include <linux/magic.h>
#include <sys/vfs.h>
#include <sys/xattr.h>
#define FIXTURE_FILESYSTEM "ext4"
#define FIXTURE_ARCHITECTURE "x86_64"
#else
#error "This fixture requires Apple Silicon macOS or x86_64 Linux."
#endif

#define MAX_BYTES 256
#define TOTAL_DEADLINE_SECONDS 30
#define CHILD_PHASE_SECONDS 5

typedef struct {
    dev_t device;
    ino_t inode;
    off_t size;
} Identity;

typedef enum {
    CREATE_WITHOUT_REPLACEMENT,
    EXCHANGE_NAMES,
} RenameMode;

static int rename_fixture(int directory_fd, const char *source, const char *target, RenameMode mode) {
#ifdef __linux__
    unsigned int flags = mode == EXCHANGE_NAMES ? RENAME_EXCHANGE : RENAME_NOREPLACE;
    return renameat2(directory_fd, source, directory_fd, target, flags);
#else
    unsigned int flags = mode == EXCHANGE_NAMES ? RENAME_SWAP : RENAME_EXCL;
    return renameatx_np(directory_fd, source, directory_fd, target, flags);
#endif
}

static void fail(const char *label) {
    (void)fprintf(stderr, "FAIL:%s\n", label);
    exit(1);
}

static void deadline_expired(int signal_number) {
    (void)signal_number;
    static const char message[] = "FAIL:deadline\n";
    (void)write(STDERR_FILENO, message, sizeof(message) - 1);
    _exit(124);
}

static int write_all(int descriptor, const void *data, size_t length) {
    const unsigned char *bytes = (const unsigned char *)data;
    size_t offset = 0;
    while (offset < length) {
        ssize_t count = write(descriptor, bytes + offset, length - offset);
        if (count > 0) {
            offset += (size_t)count;
        } else if (count < 0 && errno == EINTR) {
            continue;
        } else {
            return 0;
        }
    }
    return 1;
}

static int replace_contents(int descriptor, const char *text) {
    size_t length = strlen(text);
    if (length > MAX_BYTES || ftruncate(descriptor, 0) != 0) {
        return 0;
    }
    size_t offset = 0;
    while (offset < length) {
        ssize_t count = pwrite(descriptor, text + offset, length - offset, (off_t)offset);
        if (count > 0) {
            offset += (size_t)count;
        } else if (count < 0 && errno == EINTR) {
            continue;
        } else {
            return 0;
        }
    }
    return 1;
}

static void create_file(int directory_fd, const char *name, const char *text) {
    int descriptor = openat(
        directory_fd,
        name,
        O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC,
        0600
    );
    if (descriptor < 0) {
        fail("fixture_create");
    }
    int ok = fchmod(descriptor, 0600) == 0 && write_all(descriptor, text, strlen(text));
    if (close(descriptor) != 0 || !ok) {
        fail("fixture_write");
    }
}

static int identity_at(int directory_fd, const char *name, Identity *identity) {
    struct stat info;
    if (fstatat(directory_fd, name, &info, AT_SYMLINK_NOFOLLOW) != 0 ||
        !S_ISREG(info.st_mode) || info.st_nlink != 1 || info.st_size < 0 ||
        info.st_size > MAX_BYTES) {
        return 0;
    }
    identity->device = info.st_dev;
    identity->inode = info.st_ino;
    identity->size = info.st_size;
    return 1;
}

static int same_identity(const Identity *left, const Identity *right) {
    return left->device == right->device && left->inode == right->inode;
}

static int read_file(
    int directory_fd,
    const char *name,
    unsigned char bytes[MAX_BYTES],
    size_t *length,
    Identity *identity
) {
    int descriptor = openat(directory_fd, name, O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
    if (descriptor < 0) {
        return 0;
    }
    struct stat before;
    if (fstat(descriptor, &before) != 0 || !S_ISREG(before.st_mode) || before.st_nlink != 1 ||
        before.st_size < 0 || before.st_size > MAX_BYTES) {
        (void)close(descriptor);
        return 0;
    }
    size_t used = 0;
    while (used < MAX_BYTES) {
        ssize_t count = read(descriptor, bytes + used, MAX_BYTES - used);
        if (count > 0) {
            used += (size_t)count;
        } else if (count == 0) {
            break;
        } else if (errno != EINTR) {
            (void)close(descriptor);
            return 0;
        }
    }
    if (used == MAX_BYTES) {
        unsigned char extra;
        if (read(descriptor, &extra, 1) != 0) {
            (void)close(descriptor);
            return 0;
        }
    }
    struct stat after;
    int stable = fstat(descriptor, &after) == 0 && before.st_dev == after.st_dev &&
        before.st_ino == after.st_ino && before.st_size == after.st_size &&
        used == (size_t)before.st_size;
    (void)close(descriptor);
    if (!stable) {
        return 0;
    }
    identity->device = before.st_dev;
    identity->inode = before.st_ino;
    identity->size = before.st_size;
    *length = used;
    return 1;
}

static int file_equals(int directory_fd, const char *name, const char *expected) {
    unsigned char bytes[MAX_BYTES];
    size_t length = 0;
    Identity identity;
    size_t expected_length = strlen(expected);
    return read_file(directory_fd, name, bytes, &length, &identity) &&
        length == expected_length && memcmp(bytes, expected, length) == 0;
}

static int file_exists(int directory_fd, const char *name) {
    struct stat info;
    return fstatat(directory_fd, name, &info, AT_SYMLINK_NOFOLLOW) == 0;
}

static int file_absent(int directory_fd, const char *name) {
    struct stat info;
    return fstatat(directory_fd, name, &info, AT_SYMLINK_NOFOLLOW) != 0 && errno == ENOENT;
}

static int wait_child(pid_t child, int *status) {
    pid_t result;
    do {
        result = waitpid(child, status, 0);
    } while (result < 0 && errno == EINTR);
    return result == child;
}

static int await_file(int directory_fd, const char *name, const char *contents) {
    struct timespec pause = {.tv_sec = 0, .tv_nsec = 5000000};
    struct timespec started;
    if (clock_gettime(CLOCK_MONOTONIC, &started) != 0) {
        return 0;
    }
    for (;;) {
        if (file_equals(directory_fd, name, contents)) {
            return 1;
        }
        struct timespec now;
        if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) {
            return 0;
        }
        int64_t elapsed_ms = (int64_t)(now.tv_sec - started.tv_sec) * 1000 +
            (int64_t)(now.tv_nsec - started.tv_nsec) / 1000000;
        if (elapsed_ms >= CHILD_PHASE_SECONDS * 1000) {
            return 0;
        }
        (void)nanosleep(&pause, NULL);
    }
}

static int directory_is_empty(int directory_fd) {
    int duplicate = dup(directory_fd);
    if (duplicate < 0) {
        return 0;
    }
    DIR *directory = fdopendir(duplicate);
    if (directory == NULL) {
        (void)close(duplicate);
        return 0;
    }
    int empty = 1;
    struct dirent *entry;
    while ((entry = readdir(directory)) != NULL) {
        if (strcmp(entry->d_name, ".") != 0 && strcmp(entry->d_name, "..") != 0) {
            empty = 0;
            break;
        }
    }
    (void)closedir(directory);
    return empty;
}

static int valid_private_cwd(int directory_fd) {
    struct stat info;
    struct statfs filesystem;
    if (fstat(directory_fd, &info) != 0 || !S_ISDIR(info.st_mode) ||
        info.st_uid != geteuid() || (info.st_mode & 07777) != 0700 ||
        fstatfs(directory_fd, &filesystem) != 0) {
        return 0;
    }
#ifdef __linux__
    int supported = geteuid() != 0 && filesystem.f_type == EXT4_SUPER_MAGIC;
#else
    int supported = strcmp(filesystem.f_fstypename, "apfs") == 0 &&
        (filesystem.f_flags & MNT_LOCAL) != 0;
#endif
    return supported && directory_is_empty(directory_fd);
}

static int fixture_file_identity(int descriptor, Identity *identity) {
    struct stat info;
    if (fstat(descriptor, &info) != 0 || !S_ISREG(info.st_mode) || info.st_nlink != 1) {
        return 0;
    }
    identity->device = info.st_dev;
    identity->inode = info.st_ino;
    identity->size = info.st_size;
    return 1;
}

static void child_in_place_write(int directory_fd, int inherited_lock) {
    (void)close(inherited_lock);
    int descriptor = openat(directory_fd, "config", O_RDWR | O_NOFOLLOW | O_CLOEXEC);
    if (descriptor < 0) {
        _exit(51);
    }
    int result = flock(descriptor, LOCK_EX | LOCK_NB);
    if (result == 0) {
        (void)flock(descriptor, LOCK_UN);
        _exit(52);
    }
    if (errno != EWOULDBLOCK && errno != EAGAIN) {
        _exit(53);
    }
    int wrote = replace_contents(descriptor, "child-in-place");
    (void)close(descriptor);
    _exit(wrote ? 0 : 54);
}

static void run_child_exit(pid_t child, const char *label) {
    int status = 0;
    if (!wait_child(child, &status) || !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        fail(label);
    }
}

static void advisory_flock_and_path_operations(int directory_fd) {
    create_file(directory_fd, "config", "parent-original");
    create_file(directory_fd, "replacement", "atomic-replacement");
    int locked = openat(directory_fd, "config", O_RDWR | O_NOFOLLOW | O_CLOEXEC);
    if (locked < 0 || flock(locked, LOCK_EX | LOCK_NB) != 0) {
        fail("parent_flock");
    }
    Identity locked_identity;
    if (!fixture_file_identity(locked, &locked_identity)) {
        fail("locked_file_identity");
    }

    pid_t child = fork();
    if (child < 0) {
        fail("flock_child");
    }
    if (child == 0) {
        child_in_place_write(directory_fd, locked);
    }
    run_child_exit(child, "advisory_flock_or_in_place_write");
    if (!file_equals(directory_fd, "config", "child-in-place")) {
        fail("in_place_write_observation");
    }

    child = fork();
    if (child < 0) {
        fail("rename_child");
    }
    if (child == 0) {
        _exit(renameat(directory_fd, "replacement", directory_fd, "config") == 0 ? 0 : 61);
    }
    run_child_exit(child, "atomic_replace_child");
    Identity replacement_identity;
    struct stat still_locked;
    if (!file_equals(directory_fd, "config", "atomic-replacement") ||
        !identity_at(directory_fd, "config", &replacement_identity) ||
        same_identity(&locked_identity, &replacement_identity) || fstat(locked, &still_locked) != 0 ||
        still_locked.st_nlink != 0) {
        fail("atomic_replace_observation");
    }

    (void)close(locked);
    locked = openat(directory_fd, "config", O_RDWR | O_NOFOLLOW | O_CLOEXEC);
    Identity unlink_identity;
    if (locked < 0 || flock(locked, LOCK_EX | LOCK_NB) != 0 ||
        !fixture_file_identity(locked, &unlink_identity) ||
        !same_identity(&replacement_identity, &unlink_identity)) {
        fail("lock_replacement_before_unlink");
    }
    child = fork();
    if (child < 0) {
        fail("unlink_child");
    }
    if (child == 0) {
        _exit(unlinkat(directory_fd, "config", 0) == 0 ? 0 : 62);
    }
    run_child_exit(child, "unlink_child");
    if (!file_absent(directory_fd, "config") || fstat(locked, &still_locked) != 0 ||
        still_locked.st_nlink != 0) {
        fail("unlink_observation");
    }
    (void)flock(locked, LOCK_UN);
    (void)close(locked);
}

static void racing_user_writer(int directory_fd) {
    int temporary = openat(directory_fd, "race-temp", O_RDWR | O_NOFOLLOW | O_CLOEXEC);
    if (temporary < 0 || !replace_contents(temporary, "racing-user-file") ||
        close(temporary) != 0 ||
        renameat(directory_fd, "race-temp", directory_fd, "race-target") != 0) {
        _exit(71);
    }
    int held = openat(directory_fd, "race-target", O_RDWR | O_NOFOLLOW | O_CLOEXEC);
    Identity held_identity;
    if (held < 0 || !fixture_file_identity(held, &held_identity)) {
        _exit(72);
    }
    int ready = openat(
        directory_fd,
        "race-ready",
        O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC,
        0600
    );
    if (ready < 0 || !write_all(ready, "ready", 5) || close(ready) != 0) {
        _exit(73);
    }
    struct timespec pause = {.tv_sec = 0, .tv_nsec = 5000000};
    struct timespec started;
    if (clock_gettime(CLOCK_MONOTONIC, &started) != 0) {
        _exit(74);
    }
    struct timespec now;
    for (;;) {
        if (file_exists(directory_fd, "race-go")) {
            break;
        }
        if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) {
            _exit(75);
        }
        int64_t elapsed_ms = (int64_t)(now.tv_sec - started.tv_sec) * 1000 +
            (int64_t)(now.tv_nsec - started.tv_nsec) / 1000000;
        if (elapsed_ms >= CHILD_PHASE_SECONDS * 1000) {
            _exit(76);
        }
        (void)nanosleep(&pause, NULL);
    }
    if (clock_gettime(CLOCK_MONOTONIC, &started) != 0) {
        _exit(77);
    }
    int wrote = replace_contents(held, "racing-file-written-after-swap");
    (void)close(held);
    _exit(wrote ? 0 : 79);
}

static void exclusive_and_swap_checks(int directory_fd) {
    create_file(directory_fd, "exclusive-source", "new-entry");
    Identity create_before;
    Identity create_after;
    if (!identity_at(directory_fd, "exclusive-source", &create_before) ||
        rename_fixture(directory_fd, "exclusive-source", "exclusive-created", CREATE_WITHOUT_REPLACEMENT) != 0 ||
        !file_absent(directory_fd, "exclusive-source") ||
        !identity_at(directory_fd, "exclusive-created", &create_after) ||
        !same_identity(&create_before, &create_after) ||
        !file_equals(directory_fd, "exclusive-created", "new-entry")) {
        fail("rename_excl_create");
    }

    create_file(directory_fd, "exclusive-source-2", "source-stays");
    create_file(directory_fd, "exclusive-target-2", "target-stays");
    Identity source_before;
    Identity target_before;
    if (!identity_at(directory_fd, "exclusive-source-2", &source_before) ||
        !identity_at(directory_fd, "exclusive-target-2", &target_before)) {
        fail("rename_excl_precheck");
    }
    errno = 0;
    int result = rename_fixture(
        directory_fd,
        "exclusive-source-2",
        "exclusive-target-2",
        CREATE_WITHOUT_REPLACEMENT
    );
    int rename_error = errno;
    Identity source_after;
    Identity target_after;
    if (result == 0 || rename_error != EEXIST ||
        !identity_at(directory_fd, "exclusive-source-2", &source_after) ||
        !identity_at(directory_fd, "exclusive-target-2", &target_after) ||
        !same_identity(&source_before, &source_after) ||
        !same_identity(&target_before, &target_after) ||
        !file_equals(directory_fd, "exclusive-source-2", "source-stays") ||
        !file_equals(directory_fd, "exclusive-target-2", "target-stays")) {
        fail("rename_excl_eexist_preserves_both");
    }

    create_file(directory_fd, "race-target", "original-config");
    create_file(directory_fd, "race-temp", "temporary-placeholder");
    create_file(directory_fd, "race-candidate", "candidate-config");
    Identity original;
    Identity candidate_before;
    if (!identity_at(directory_fd, "race-candidate", &candidate_before) ||
        !identity_at(directory_fd, "race-target", &original) ||
        !file_equals(directory_fd, "race-target", "original-config")) {
        fail("race_precheck");
    }
    pid_t racer = fork();
    if (racer < 0) {
        fail("race_child");
    }
    if (racer == 0) {
        racing_user_writer(directory_fd);
    }
    if (!await_file(directory_fd, "race-ready", "ready")) {
        fail("race_writer_opened_inode");
    }
    Identity racing;
    if (!identity_at(directory_fd, "race-target", &racing) || same_identity(&original, &racing) ||
        !file_equals(directory_fd, "race-target", "racing-user-file")) {
        fail("postcheck_racing_replacement");
    }
    if (rename_fixture(directory_fd, "race-candidate", "race-target", EXCHANGE_NAMES) != 0) {
        fail("rename_swap");
    }
    Identity displaced;
    Identity published;
    if (!identity_at(directory_fd, "race-candidate", &displaced) ||
        !same_identity(&racing, &displaced) ||
        !file_equals(directory_fd, "race-candidate", "racing-user-file") ||
        !identity_at(directory_fd, "race-target", &published) ||
        !same_identity(&candidate_before, &published) ||
        !file_equals(directory_fd, "race-target", "candidate-config")) {
        fail("swap_displaced_racing_inode_and_bytes");
    }
    create_file(directory_fd, "race-go", "go");
    run_child_exit(racer, "displaced_inode_writer");
    Identity writable_displaced;
    if (!identity_at(directory_fd, "race-candidate", &writable_displaced) ||
        !same_identity(&racing, &writable_displaced) ||
        !file_equals(directory_fd, "race-candidate", "racing-file-written-after-swap")) {
        fail("displaced_inode_remains_writable");
    }
    if (rename_fixture(directory_fd, "race-candidate", "race-target", EXCHANGE_NAMES) != 0) {
        fail("swap_back");
    }
    Identity restored;
    Identity candidate;
    if (!identity_at(directory_fd, "race-target", &restored) ||
        !same_identity(&racing, &restored) ||
        !file_equals(directory_fd, "race-target", "racing-file-written-after-swap") ||
        !identity_at(directory_fd, "race-candidate", &candidate) ||
        !same_identity(&candidate_before, &candidate) ||
        !file_equals(directory_fd, "race-candidate", "candidate-config")) {
        fail("swap_back_preserves_racing_inode_and_bytes");
    }
}

#ifdef __linux__
static int read_birth_time(int fd, struct statx_timestamp *birth) {
    struct statx info;
    if (statx(fd, "", AT_EMPTY_PATH, STATX_BTIME, &info) != 0 ||
        (info.stx_mask & STATX_BTIME) == 0) {
        return 0;
    }
    *birth = info.stx_btime;
    return 1;
}

static void linux_metadata_publication(int directory_fd) {
    static const char attribute[] = "user.pennypinch.fixture";
    static const char value[] = "preserve";
    char buffer[sizeof(value)];
    create_file(directory_fd, "metadata-target", "old-file");
    create_file(directory_fd, "metadata-candidate", "new-file");
    int original = openat(directory_fd, "metadata-target", O_RDWR | O_NOFOLLOW | O_CLOEXEC);
    int candidate = openat(directory_fd, "metadata-candidate", O_RDWR | O_NOFOLLOW | O_CLOEXEC);
    struct stat original_info;
    if (original < 0 || candidate < 0 ||
        fchmod(original, 0640) != 0 ||
        fsetxattr(original, attribute, value, sizeof(value), 0) != 0 ||
        fstat(original, &original_info) != 0 ||
        fgetxattr(original, attribute, buffer, sizeof(buffer)) != (ssize_t)sizeof(value) ||
        fchmod(candidate, original_info.st_mode & 0777) != 0 ||
        fsetxattr(candidate, attribute, buffer, sizeof(buffer), 0) != 0) {
        fail("linux_metadata_stage");
    }

    struct statx_timestamp before;
    Identity identity_before;
    if (!read_birth_time(candidate, &before) || !fixture_file_identity(candidate, &identity_before)) {
        fail("linux_candidate_identity");
    }
    if (fsync(candidate) != 0 || fsync(directory_fd) != 0 ||
        rename_fixture(directory_fd, "metadata-candidate", "metadata-target", EXCHANGE_NAMES) != 0 ||
        fsync(directory_fd) != 0) {
        fail("linux_metadata_publication");
    }

    int published = openat(directory_fd, "metadata-target", O_RDONLY | O_NOFOLLOW | O_CLOEXEC);
    struct stat published_info;
    struct statx_timestamp after;
    Identity identity_after;
    if (published < 0 || fstat(published, &published_info) != 0 ||
        published_info.st_mode != original_info.st_mode ||
        published_info.st_uid != original_info.st_uid || published_info.st_gid != original_info.st_gid ||
        !read_birth_time(published, &after) ||
        before.tv_sec != after.tv_sec || before.tv_nsec != after.tv_nsec ||
        !fixture_file_identity(published, &identity_after) || !same_identity(&identity_before, &identity_after) ||
        fgetxattr(published, attribute, buffer, sizeof(buffer)) != (ssize_t)sizeof(value) ||
        memcmp(value, buffer, sizeof(value)) != 0 ||
        !file_equals(directory_fd, "metadata-target", "new-file") ||
        !file_equals(directory_fd, "metadata-candidate", "old-file")) {
        fail("linux_metadata_after_publication");
    }
    (void)close(published);
    (void)close(candidate);
    (void)close(original);
}
#endif

static void crash_after_swap_before_owner(int directory_fd) {
    create_file(directory_fd, "crash-config", "config-before");
    create_file(directory_fd, "crash-next", "config-after");
    create_file(directory_fd, "crash-owner", "owner-before");
    Identity owner_before;
    if (!identity_at(directory_fd, "crash-owner", &owner_before)) {
        fail("owner_precheck");
    }
    pid_t child = fork();
    if (child < 0) {
        fail("crash_child");
    }
    if (child == 0) {
        if (rename_fixture(directory_fd, "crash-next", "crash-config", EXCHANGE_NAMES) != 0) {
            _exit(91);
        }
#ifdef __linux__
        if (fsync(directory_fd) != 0) {
            _exit(93);
        }
#endif
        (void)kill(getpid(), SIGKILL);
        int owner = openat(directory_fd, "crash-owner", O_WRONLY | O_NOFOLLOW | O_CLOEXEC);
        if (owner >= 0) {
            (void)replace_contents(owner, "owner-after");
            (void)close(owner);
        }
        _exit(92);
    }
    int status = 0;
    if (!wait_child(child, &status) || !WIFSIGNALED(status) || WTERMSIG(status) != SIGKILL) {
        fail("child_killed_after_config_swap");
    }
    Identity owner_after;
    if (!file_equals(directory_fd, "crash-config", "config-after") ||
        !file_equals(directory_fd, "crash-next", "config-before") ||
        !file_equals(directory_fd, "crash-owner", "owner-before") ||
        !identity_at(directory_fd, "crash-owner", &owner_after) ||
        !same_identity(&owner_before, &owner_after)) {
        fail("crash_window_observation");
    }
}

int main(int argc, char **argv) {
    (void)argv;
    if (argc != 1) {
        fail("no_arguments_allowed");
    }
    struct sigaction action;
    memset(&action, 0, sizeof(action));
    action.sa_handler = deadline_expired;
    sigemptyset(&action.sa_mask);
    if (sigaction(SIGALRM, &action, NULL) != 0) {
        fail("deadline_handler");
    }
    (void)alarm(TOTAL_DEADLINE_SECONDS);

    int directory_fd = open(".", O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
    if (directory_fd < 0 || !valid_private_cwd(directory_fd)) {
        fail("private_empty_local_fixture");
    }
    advisory_flock_and_path_operations(directory_fd);
    exclusive_and_swap_checks(directory_fd);
#ifdef __linux__
    linux_metadata_publication(directory_fd);
#endif
    crash_after_swap_before_owner(directory_fd);

    (void)alarm(0);
    if (printf(
            "{\"filesystem\":\"" FIXTURE_FILESYSTEM "\",\"architecture\":\"" FIXTURE_ARCHITECTURE "\","
            "\"cwd_private_0700\":true,\"advisory_flock_rejected\":true,"
            "\"in_place_write_succeeded\":true,\"atomic_replacement_succeeded\":true,"
            "\"unlink_succeeded\":true,\"rename_excl_create\":true,"
            "\"rename_excl_eexist_preserves_both\":true,"
            "\"postcheck_racing_replacement_observed\":true,"
            "\"swap_displaced_racing_inode_and_bytes\":true,"
            "\"displaced_inode_remains_writable\":true,"
            "\"swap_back_preserved_racing_inode_and_bytes\":true,"
            "\"child_killed_after_config_swap_before_owner_publication\":true,"
            "\"displaced_original_and_owner_unchanged\":true"
#ifdef __linux__
            ",\"staged_mode_and_user_xattr_preserved\":true"
            ",\"birth_time_and_inode_preserved\":true"
            ",\"file_and_directory_sync_succeeded\":true"
#endif
            "}\n"
        ) < 0 || fflush(stdout) != 0) {
        fail("json_output");
    }
    (void)close(directory_fd);
    return 0;
}
