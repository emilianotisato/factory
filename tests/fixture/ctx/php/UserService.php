<?php

namespace App\Services;

use App\Models\User;

/**
   Registers users. The welcome mail goes through the
   class OtherService, which this one wraps.
 */
final class UserService
{
    use HasTimestamps;

    public function __construct(private OtherService $mail)
    {
    }

    public function handle(array $input): User
    {
        $user = User::create($input);
        $this->mail->welcome($user);
        $this->touch();

        return $user;
    }
}
